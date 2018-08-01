extern crate futures;
extern crate http;
extern crate hyper;
extern crate regex;
extern crate serde;
#[macro_use]
extern crate serde_derive;
extern crate serde_json;

use std::any::{Any, TypeId};
use std::borrow::Cow;
use std::collections::HashMap;
use std::sync::Arc;

use futures::{future, Future};
use http::{request, response};
pub use hyper::StatusCode;
pub use hyper::header::{self, HeaderMap, HeaderValue};
use hyper::service::{NewService, Service};
use hyper::{rt, Body, Method, Server, Uri, Version};
use regex::Regex;
use serde::Serialize;

pub struct Direkuta {
    // TODO: Rename to state?
    context: Arc<Context>,
    /// Stores middleware, to be later used in `Service::call(...)`.
    middle: Arc<HashMap<TypeId, Box<Herupa + Send + Sync + 'static>>>,
    routes: Arc<RouteRecognizer>,
}

impl Direkuta {
    /// Constructs a new `Direkuta`
    /// 
    /// # Examples
    /// ```
    /// use direkuta::Direkuta;
    /// 
    /// let dire = Direkuta::new();
    /// ```
    pub fn new() -> Self {
        Default::default()
    }

    /// Insert a state into `Direkuta`
    /// 
    /// # Examples
    /// ```
    /// use direkuta::Direkuta;
    /// 
    /// let dire = Direkuta::new();
    /// ```
    /// 
    /// # Panics
    /// Do not use this from anywhere else but the main constructor.
    /// Using this from any else will cause tread panic.
    pub fn state<T: Any + Send + Sync + 'static>(mut self, state: T) -> Self {
        Arc::get_mut(&mut self.context)
            .expect("Cannot get_mut on state")
            .set(state);
        self
    }

    /// Insert a middleware into `Direkuta`
    /// 
    /// Middleware is anything that impliments the trait `Herupa`.
    /// 
    /// # Examples
    /// ```
    /// use direkuta::{Direkuta, Logger};
    /// 
    /// let dire = Direkuta::new()
    ///     .middle(Logger::new());
    /// ```
    /// 
    /// # Panics
    /// Do not use this from anywhere else but the main constructor.
    /// Using this from any else will cause tread panic.
    pub fn middle<T: Herupa + Send + Sync + 'static>(mut self, middle: T) -> Self {
        Arc::get_mut(&mut self.middle)
            .expect("Cannot get_mut on middle")
            .insert(TypeId::of::<T>(), Box::new(middle));
        self
    }

    pub fn route<R: Fn(&mut RouteBuilder) + Send + Sync + 'static>(mut self, route: R) -> Self {
        let mut route_builder = RouteBuilder {
            routes: HashMap::new(),
        };

        route(&mut route_builder);
        self.routes = Arc::new(route_builder.finish());

        self
    }

    /// Run `Direkuta` as a Hyper server.
    /// 
    /// # Examples
    /// ```
    /// use direkuta::Direkuta;
    /// 
    /// let dire = Direkuta::new()
    ///     .route(|r| {
    ///         r.get("/", |c| {
    ///         });
    ///     })
    ///     .run("0.0.0.0:8001");
    /// ```
    /// 
    /// # Errors
    /// If any errors come from the server they will be printed to the console.
    pub fn run(self, address: &str) {
        let address = address.parse()
            .expect("Address not a valid socket address");
        let server = Server::bind(&address)
            .serve(self)
            .map_err(|e| eprintln!("server error: {}", e));

        rt::run(server);
    }
}

impl Default for Direkuta {
    fn default() -> Self {
        Self {
            context: Arc::new(Context::new()),
            middle: Arc::new(HashMap::new()),
            routes: Arc::new(RouteRecognizer {
                routes: HashMap::new(),
            }),
        }
    }
}

impl NewService for Direkuta {
    type ReqBody = Body;
    type ResBody = Body;
    type Error = hyper::Error;
    type InitError = hyper::Error;
    type Service = Direkuta;
    type Future = Box<Future<Item = Self::Service, Error = Self::InitError> + Send>;

    fn new_service(&self) -> Self::Future {
        Box::new(future::ok(Self {
            context: self.context.clone(),
            middle: self.middle.clone(),
            routes: self.routes.clone(),
        }))
    }
}

impl Service for Direkuta {
    type ReqBody = Body;
    type ResBody = Body;
    type Error = hyper::Error;
    type Future = Box<Future<Item = response::Response<Self::ReqBody>, Error = Self::Error> + Send>;

    fn call(&mut self, req: request::Request<Self::ReqBody>) -> Self::Future {
        let method = req.method().clone();
        let path = req.uri().path().to_owned();
        let (parts, body) = req.into_parts();
        let req = Request::new(body, parts);

        let res: Response = match self.routes.recognize(&method, &path) {
            Ok((handler, cap)) => handler(
                &req,
                &self.context.clone(),
                &cap,
            ),
            Err(code) => {
                let mut res = Response::new();
                res.set_status(code.as_u16());
                res
            }
        };

        Box::new(future::ok(res.into_hyper()))
    }
}

/// All middleware must implement this trait.
/// 
/// # Examples
/// ```
/// use direkuta::Herupa;
/// 
/// struct Logger {}
/// 
/// impl Logger {
///     pub fn new() -> Self {
///         Self { }
///     }
/// }
/// 
/// impl Herupa for Logger {
///     fn before(&self, req: &mut Request) {
///         println!("[{}] `{}`", req.method(), req.uri());
///     }
/// 
///     fn after(&self, req: &mut Request, res: &mut Response) {
///         println!("[{}] `{}`", res.status(), req.uri());
///     }
/// }
pub trait Herupa {
    /// Called before a request is sent through `RouteRecognizer`
    fn before(&self, &mut Request);
    /// Called after a request is sent through `RouteRecognizer`
    fn after(&self, &mut Request, &mut Response);
}

/// A simple logger middleware.
/// 
/// # Examples
/// ```
/// use direkuta::{Direkuta, Logger};
/// 
/// let dire = Direkuta::new()
///     .middle(Logger::new());
/// ```
pub struct Logger {}

impl Logger {
    /// Constructs a new `Logger`
    pub fn new() -> Self {
        Self { }
    }
}

impl Herupa for Logger {
    fn before(&self, req: &mut Request) {
        println!("[{}] `{}`", req.method(), req.uri());
    }

    fn after(&self, req: &mut Request, res: &mut Response) {
        println!("[{}] `{}`", res.status(), req.uri());
    }
}

pub struct Context {
    inner: HashMap<TypeId, Box<Any + Send + Sync + 'static>>,
}

impl Context {
    /// Constructs a new `Context`
    pub fn new() -> Self {
        Self {
            inner: HashMap::new(),
        }
    }

    pub fn set<T: Any + Send + Sync + 'static>(&mut self, ctx: T) {
        self.inner.insert(TypeId::of::<T>(), Box::new(ctx));
    }

    pub fn try_get<T: Any + Send + Sync + 'static>(&self) -> Option<&T> {
        self.inner
            .get(&TypeId::of::<T>())
            .and_then(|b| Any::downcast_ref::<T>(b))
    }

    pub fn get<T: Any + Send + Sync + 'static>(&self) -> &T {
        self.try_get::<T>().expect("Key not found is context")
    }
}

pub struct Route {
    handler: Box<Fn(&Request, &Context, &Captures) -> Response + Send + Sync + 'static>,
    pattern: Regex,
}

pub struct RouteBuilder {
    routes: HashMap<Method, Vec<Route>>,
}

impl RouteBuilder {
    fn route<S: AsRef<str>, H: Fn(&Request, &Context, &Captures) -> Response + Send + Sync + 'static>(
        &mut self,
        method: Method,
        pattern: S,
        handler: H,
    ) -> &Self {
        let pattern = normalize_pattern(pattern.as_ref());
        let pattern = Regex::new(&pattern).expect("Pattern does not contain valid regex");
        let handler = Box::new(handler);
        self.routes
            .entry(method)
            .or_insert(Vec::new())
            .push(Route { handler, pattern });
        self
    }

    fn finish(self) -> RouteRecognizer {
        RouteRecognizer {
            routes: self.routes,
        }
    }

    pub fn get<S: AsRef<str>, H: Fn(&Request, &Context, &Captures) -> Response + Send + Sync + 'static>(
        &mut self,
        pattern: S,
        handler: H,
    ) -> &Self {
        self.route(Method::GET, pattern, handler)
    }

    pub fn post<S: AsRef<str>, H: Fn(&Request, &Context, &Captures) -> Response + Send + Sync + 'static>(
        &mut self,
        pattern: S,
        handler: H,
    ) -> &Self {
        self.route(Method::POST, pattern, handler)
    }

    pub fn put<S: AsRef<str>, H: Fn(&Request, &Context, &Captures) -> Response + Send + Sync + 'static>(
        &mut self,
        pattern: S,
        handler: H,
    ) -> &Self {
        self.route(Method::PUT, pattern, handler)
    }

    pub fn delete<S: AsRef<str>, H: Fn(&Request, &Context, &Captures) -> Response + Send + Sync + 'static>(
        &mut self,
        pattern: S,
        handler: H,
    ) -> &Self {
        self.route(Method::DELETE, pattern, handler)
    }

    pub fn head<S: AsRef<str>, H: Fn(&Request, &Context, &Captures) -> Response + Send + Sync + 'static>(
        &mut self,
        pattern: S,
        handler: H,
    ) -> &Self {
        self.route(Method::HEAD, pattern, handler)
    }

    pub fn options<S: AsRef<str>, H: Fn(&Request, &Context, &Captures) -> Response + Send + Sync + 'static>(
        &mut self,
        pattern: S,
        handler: H,
    ) -> &Self {
        self.route(Method::OPTIONS, pattern, handler)
    }
}

type Captures = Vec<(Option<String>, String)>;

pub struct RouteRecognizer {
    routes: HashMap<Method, Vec<Route>>,
}

impl RouteRecognizer {
    fn recognize(
        &self,
        method: &Method,
        path: &str,
    ) -> Result<
        (
            &(Fn(&Request, &Context, &Captures) -> Response + Send + Sync + 'static),
            Captures,
        ),
        StatusCode,
    > {
        let routes = self.routes.get(method).ok_or(StatusCode::NOT_FOUND)?;
        for route in routes {
            if let Some(caps) = get_owned_captures(&route.pattern, path) {
                return Ok((&*route.handler, caps));
            }
        }
        Err(StatusCode::NOT_FOUND)
    }
}

fn get_owned_captures(re: &Regex, path: &str) -> Option<Captures> {
    re.captures(path).map(|caps| {
        let mut res = Vec::with_capacity(caps.len());
        for (i, name) in re.capture_names().enumerate() {
            let val = match name {
                Some(name) => caps.name(name).unwrap(),
                None => caps.get(i).unwrap(),
            };
            res.push((name.map(|s| s.to_owned()), val.as_str().to_owned()));
        }
        res
    })
}

fn normalize_pattern(pattern: &str) -> Cow<str> {
    let pattern = pattern
        .trim()
        .trim_left_matches("^")
        .trim_right_matches("$")
        .trim_right_matches("/");
    match pattern {
        "" => "^/$".into(),
        s => format!("^{}/?$", s).into(),
    }
}

pub struct Response {
    pub(crate) body: Body,
    pub(crate) parts: response::Parts,
}

impl Response {
    /// Constructs a new `Response`
    pub fn new() -> Self {
        Response::default()
    }

    pub fn version(&self) -> Version {
        self.parts.version
    }

    pub fn headers(&self) -> &HeaderMap<HeaderValue> {
        &self.parts.headers
    }

    pub fn headers_mut(&mut self) -> &mut HeaderMap<HeaderValue> {
        &mut self.parts.headers
    }

    pub fn set_headers(&mut self, headers: HeaderMap<HeaderValue>) {
        self.parts.headers = headers;
    }

    pub fn status(&self) -> StatusCode {
        self.parts.status
    }

    pub fn status_mut(&mut self) -> &mut StatusCode {
        &mut self.parts.status
    }

    pub fn set_status(&mut self, status: u16) {
        self.parts.status = StatusCode::from_u16(status)
            .expect("Given status is not a valid status code");
    }

    pub fn with_status(mut self, status: u16) -> Self {
        self.set_status(status);
        self
    }

    pub fn body(self) -> Body {
        self.body
    }

    pub fn set_body<T: Into<String>>(&mut self, body: T) {
        let body = body.into();
        self.headers_mut().insert(
            header::CONTENT_LENGTH,
            HeaderValue::from_str(&body.len().to_string()).expect("Given value for CONTENT_LENGTH is not valid"),
        );
        self.body = Body::from(body);
    }

    pub fn redirect(mut self, url: &'static str) -> Response {
        self.set_status(301);
        self.headers_mut()
            .insert(header::LOCATION, HeaderValue::from_static(url));

        self
    }

    pub fn error(&mut self, messages: Vec<&str>) {
        self.headers_mut().insert(
            header::CONTENT_TYPE,
            HeaderValue::from_static("application/json"),
        );
        
        let mut wrapper: Wrapper<()> = Wrapper::new();
        wrapper.set_code(self.parts.status.as_u16());
        wrapper.set_status(&self.parts.status.as_str());

        for message in messages {
            wrapper.add_message(message);
        }

        let json = serde_json::to_string(&wrapper)
            .expect("Can not transform strcut into json");
        self.set_body(json);
    }

    pub fn json<J: Serialize + Send + Sync + 'static>(&mut self, json: J) {
        self.headers_mut().insert(
            header::CONTENT_TYPE,
            HeaderValue::from_static("application/json"),
        );
    
        let mut wrapper: Wrapper<J> = Wrapper::new();
        wrapper.set_code(self.parts.status.as_u16());
        wrapper.set_status(&self.parts.status.as_str());
        wrapper.set_result(json);

        let json = serde_json::to_string(&wrapper)
            .expect("Can not transform strcut into json");
        self.set_body(json);
    }

    pub fn into_hyper(self) -> hyper::Response<Body> {
        hyper::Response::from_parts(self.parts, self.body)
    }
}

impl Default for Response {
    fn default() -> Response {
        let (parts, body) = hyper::Response::new(Body::empty()).into_parts();
        Response {
            body,
            parts,
        }
    }
}

pub struct Request {
    pub(crate) body: Body,
    pub(crate) parts: request::Parts,
}

impl Request {
    /// Constructs a new `Request`
    pub fn new(body: Body, parts: request::Parts) -> Self {
        Self { body, parts }
    }

    pub fn version(&self) -> Version {
        self.parts.version
    }

    pub fn headers(&self) -> &HeaderMap<HeaderValue> {
        &self.parts.headers
    }

    pub fn method(&self) -> &Method {
        &self.parts.method
    }

    pub fn uri(&self) -> &Uri {
        &self.parts.uri
    }

    pub fn path(&self) -> &str {
        self.parts.uri.path()
    }

    pub fn body(&mut self) -> Body {
        ::std::mem::replace(&mut self.body, Body::empty())
    }
}

#[derive(Serialize)]
struct Wrapper<T: Serialize + Send + Sync> {
    code: u16,
    messages: Vec<String>,
    result: Option<T>,
    status: String,
}

impl<T: Serialize + Send + Sync> Wrapper<T> {
    /// Constructs a new `Wrapper<T>`
    pub fn new() -> Wrapper<T> {
        Wrapper::default()
    }

    pub fn messages(&self) -> &Vec<String> {
        &self.messages
    }

    pub fn add_message(&mut self, message: &str) {
        self.messages.push(String::from(message));
    }

    pub fn code(&self) -> &u16 {
        &self.code
    }

    pub fn set_code(&mut self, code: u16) {
        self.code = code;
    }

    pub fn status(&self) -> &String {
        &self.status
    }

    pub fn set_status(&mut self, status: &str) {
        self.status = String::from(status);
    }

    pub fn set_result(&mut self, result: T) {
        self.result = Some(result);
    }
}

impl<T: Serialize + Send + Sync> Default for Wrapper<T> {
    fn default() -> Wrapper<T> {
        Self {
            code: 200,
            messages: Vec::new(),
            result: None,
            status: String::from("OK"),
        }
    }
}

