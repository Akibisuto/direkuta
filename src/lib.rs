//! A web framework built around Hyper.
//!
//! # Examples
//! ```
//! Direkuta::new()
//!     .route(|r| {
//!         r.get("/", |_, _, _| {
//!             let mut res = Response::new();
//!             res.set_body(String::from("Hello World!"));
//!             res
//!         });
//!     })
//!     .run("0.0.0.0:3000");
//! ```

#![deny(
    missing_docs,
    single_use_lifetimes,
    trivial_casts,
    trivial_numeric_casts,
    unsafe_code,
    unstable_features,
    unused_import_braces,
    unused_qualifications,
    unreachable_pub,
    unused_results
)]

extern crate futures;
extern crate http;
extern crate hyper;
extern crate regex;
extern crate serde;
#[macro_use]
extern crate serde_derive;
extern crate serde_json;

#[cfg(feature = "template")]
extern crate tera;

use std::any::{Any, TypeId};
use std::borrow::Cow;
use std::collections::HashMap;
use std::sync::Arc;

use futures::{future, Future};
use http::{request, response};
pub use hyper::header::{self, HeaderMap, HeaderValue};
use hyper::service::{NewService, Service};
use hyper::{rt, Method, Server, Uri, Version};
pub use hyper::{Body, StatusCode};
use regex::Regex;
use serde::Serialize;

/// The Direkuta web server itself.
pub struct Direkuta {
    /// Store state as its own type.
    state: Arc<State>,
    /// Stores middleware, to be later used in `Service::call(...)`.
    middle: Arc<HashMap<TypeId, Box<Herupa + Send + Sync + 'static>>>,
    /// The router, it know where a url is meant to go.
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
        Direkuta::default()
    }

    /// Insert a state into `Direkuta`
    ///
    /// # Examples
    /// ```
    /// use direkuta::Direkuta;
    ///
    /// Direkuta::new()
    ///     ...
    /// ```
    ///
    /// # Panics
    /// Do not use this from anywhere else but the main constructor.
    /// Using this from any else will cause tread panic.
    pub fn state<T: Any + Send + Sync + 'static>(mut self, state: T) -> Self {
        Arc::get_mut(&mut self.state)
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
    /// use direkuta::*;
    ///
    /// Direkuta::new()
    ///     .middle(Logger::new())
    ///     ...
    /// ```
    ///
    /// # Panics
    /// Do not use this from anywhere else but the main constructor.
    /// Using this from any else will cause tread panic.
    pub fn middle<T: Herupa + Send + Sync + 'static>(mut self, middle: T) -> Self {
        let _ = Arc::get_mut(&mut self.middle)
            .expect("Cannot get_mut on middle")
            .insert(TypeId::of::<T>(), Box::new(middle));
        self
    }

    /// Create new router as a closure
    ///
    /// # Examples
    /// ```
    /// use direkuta::*;
    ///
    /// Direkuta::new()
    ///     .route(|r| {
    ///         ...
    ///     })
    ///     ...
    /// ```
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
    /// Direkuta::new()
    ///     .route(|r| {
    ///         ...
    ///     })
    ///     ...
    /// ```
    ///
    /// # Errors
    /// If any errors come from the server they will be printed to the console.
    pub fn run(self, addr: &str) {
        let address = addr.parse().expect("Address not a valid socket address");
        let server = Server::bind(&address)
            .serve(self)
            .map_err(|e| eprintln!("server error: {}", e));

        println!("Direkuta listening on http://{}", addr);

        rt::run(server);
    }
}

impl Default for Direkuta {
    fn default() -> Self {
        let mut state = State::new();

        #[cfg(feature = "template")]
        state.set(match tera::Tera::parse("templates/**/*") {
            Ok(t) => t,
            Err(e) => {
                println!("Parsing error(s): {}", e);
                ::std::process::exit(1);
            }
        });

        Self {
            state: Arc::new(state),
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
            state: self.state.clone(),
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

        for (_, before) in self.middle.iter() {
            before.before(&req);
        }

        let res: Response = match self.routes.recognize(&method, &path) {
            Ok((handler, cap)) => handler(&req, &self.state.clone(), &cap),
            Err(code) => {
                let mut res = Response::new();
                res.set_status(code.as_u16());
                res
            }
        };

        for (_, after) in self.middle.iter() {
            after.after(&req, &res);
        }

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
///     fn before(&self, req: &Request) {
///         println!("[{}] `{}`", req.method(), req.uri());
///     }
///
///     fn after(&self, req: &Request, res: & Response) {
///         println!("[{}] `{}`", res.status(), req.uri());
///     }
/// }
pub trait Herupa {
    /// Called before a request is sent through `RouteRecognizer`
    fn before(&self, &Request);
    /// Called after a request is sent through `RouteRecognizer`
    fn after(&self, &Request, &Response);
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
#[derive(Clone, Copy, Debug)]
pub struct Logger {}

impl Logger {
    /// Constructs a new `Logger`
    pub fn new() -> Self {
        Self {}
    }
}

impl Herupa for Logger {
    fn before(&self, req: &Request) {
        println!("[{}] `{}`", req.method(), req.uri());
    }

    fn after(&self, req: &Request, res: &Response) {
        println!("[{}] `{}`", res.status(), req.uri());
    }
}

/// A wrapper around HashMap<TypeId, Any>, used to store Direkuta state.
///
/// Stored state cannot be dynamically create and must be static.
#[derive(Debug)]
pub struct State {
    inner: HashMap<TypeId, Box<Any + Send + Sync + 'static>>,
}

impl State {
    /// Constructs a new `State`
    ///
    /// # Examples
    /// ```
    /// let state = State::new();
    /// ```
    pub fn new() -> Self {
        State::default()
    }

    /// Sets the value of whatever type is passed.
    ///
    /// Please not that you cannot have teo of the same types,
    /// one will overwrite the other.
    ///
    /// # Examples
    /// ```
    /// # let state = State::new();
    /// state.set(String::from("Hello World!"));
    /// ```
    pub fn set<T: Any + Send + Sync + 'static>(&mut self, ctx: T) {
        let _ = self.inner.insert(TypeId::of::<T>(), Box::new(ctx));
    }

    /// Attempt to get a value based on type.
    ///
    /// Use this if you are not sure if the type exists.
    ///
    /// # Examples
    /// ```
    /// # let state = State::new();
    /// # state.set(String::from("Hello World!"));
    /// match state.get::<String>() {
    ///     Some(s) => {
    ///         println!("{}", s);
    ///     },
    ///     None => {
    ///         println!("String not found in state");
    ///     },
    /// }
    /// ```
    pub fn try_get<T: Any + Send + Sync + 'static>(&self) -> Option<&T> {
        self.inner
            .get(&TypeId::of::<T>())
            .and_then(|b| b.downcast_ref::<T>())
    }

    /// Get a value based on type.
    ///
    /// This is a wrapper around `try_get` and uses an `expect`.
    ///
    /// # Examples
    /// ```
    /// # let state = State::new();
    /// # state.set(String::from("Hello World!"));
    /// println!("{}", state.get::<String>());
    /// ```
    ///
    /// # Panics
    /// If the key does not exist the function will panic
    ///
    /// If you do not know if the type exists use `try_get`.
    pub fn get<T: Any + Send + Sync + 'static>(&self) -> &T {
        self.try_get::<T>()
            .expect(&format!("Key not found in state: {:?}", &TypeId::of::<T>()))
    }
}

impl Default for State {
    fn default() -> State {
        State {
            inner: HashMap::new(),
        }
    }
}

struct Route {
    handler: Box<Fn(&Request, &State, &Captures) -> Response + Send + Sync + 'static>,
    pattern: Regex,
}

/// Route builder.
///
/// This is not to be used directly,
/// its only used for `Direkuta.route`.
pub struct RouteBuilder {
    routes: HashMap<Method, Vec<Route>>,
}

impl RouteBuilder {
    fn route<
        S: AsRef<str>,
        H: Fn(&Request, &State, &Captures) -> Response + Send + Sync + 'static,
    >(
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

    /// Adds a `Method::GET` request handler.
    ///
    /// # Examples
    /// ```
    /// # Direkuta::new()
    /// #     .route(|r| {
    /// r.get("/", |_, _, _| {
    ///     let mut res = Response::new();
    ///     res.set_body(String::from("Hello World!"));
    ///     res
    /// });
    /// #     })
    /// #     .run("0.0.0.0:3000");
    /// ```
    pub fn get<
        S: AsRef<str>,
        H: Fn(&Request, &State, &Captures) -> Response + Send + Sync + 'static,
    >(
        &mut self,
        pattern: S,
        handler: H,
    ) -> &Self {
        self.route(Method::GET, pattern, handler)
    }

    /// Adds a `Method::POST` request handler.
    ///
    /// # Examples
    /// ```
    /// # Direkuta::new()
    /// #     .route(|r| {
    /// r.post("/", |_, _, _| {
    ///     let mut res = Response::new();
    ///     res.set_body(String::from("Hello World!"));
    ///     res
    /// });
    /// #     })
    /// #     .run("0.0.0.0:3000");
    /// ```
    pub fn post<
        S: AsRef<str>,
        H: Fn(&Request, &State, &Captures) -> Response + Send + Sync + 'static,
    >(
        &mut self,
        pattern: S,
        handler: H,
    ) -> &Self {
        self.route(Method::POST, pattern, handler)
    }

    /// Adds a `Method::PUT` request handler.
    ///
    /// # Examples
    /// ```
    /// # Direkuta::new()
    /// #     .route(|r| {
    /// r.put("/", |_, _, _| {
    ///     let mut res = Response::new();
    ///     res.set_body(String::from("Hello World!"));
    ///     res
    /// });
    /// #     })
    /// #     .run("0.0.0.0:3000");
    /// ```
    pub fn put<
        S: AsRef<str>,
        H: Fn(&Request, &State, &Captures) -> Response + Send + Sync + 'static,
    >(
        &mut self,
        pattern: S,
        handler: H,
    ) -> &Self {
        self.route(Method::PUT, pattern, handler)
    }

    /// Adds a `Method::DELETE` request handler.
    ///
    /// # Examples
    /// ```
    /// # Direkuta::new()
    /// #     .route(|r| {
    /// r.delete("/", |_, _, _| {
    ///     let mut res = Response::new();
    ///     res.set_body(String::from("Hello World!"));
    ///     res
    /// });
    /// #     })
    /// #     .run("0.0.0.0:3000");
    /// ```
    pub fn delete<
        S: AsRef<str>,
        H: Fn(&Request, &State, &Captures) -> Response + Send + Sync + 'static,
    >(
        &mut self,
        pattern: S,
        handler: H,
    ) -> &Self {
        self.route(Method::DELETE, pattern, handler)
    }

    /// Adds a `Method::HEAD` request handler.
    ///
    /// # Examples
    /// ```
    /// # Direkuta::new()
    /// #     .route(|r| {
    /// r.head("/", |_, _, _| {
    ///     let mut res = Response::new();
    ///     res.set_body(String::from("Hello World!"));
    ///     res
    /// });
    /// #     })
    /// #     .run("0.0.0.0:3000");
    /// ```
    pub fn head<
        S: AsRef<str>,
        H: Fn(&Request, &State, &Captures) -> Response + Send + Sync + 'static,
    >(
        &mut self,
        pattern: S,
        handler: H,
    ) -> &Self {
        self.route(Method::HEAD, pattern, handler)
    }

    /// Adds a `Method::OPTIONS` request handler.
    ///
    /// # Examples
    /// ```
    /// # Direkuta::new()
    /// #     .route(|r| {
    /// r.options("/", |_, _, _| {
    ///     let mut res = Response::new();
    ///     res.set_body(String::from("Hello World!"));
    ///     res
    /// });
    /// #     })
    /// #     .run("0.0.0.0:3000");
    /// ```
    pub fn options<
        S: AsRef<str>,
        H: Fn(&Request, &State, &Captures) -> Response + Send + Sync + 'static,
    >(
        &mut self,
        pattern: S,
        handler: H,
    ) -> &Self {
        self.route(Method::OPTIONS, pattern, handler)
    }

    /// Create a path for multiple request types.
    ///
    /// # Examples
    /// ```
    /// # Direkuta::new()
    /// #     .route(|r| {
    /// r.path("/", |r| {
    ///     r.get(|_, _, _| {
    ///         let mut res = Response::new();
    ///         res.set_body(String::from("Hello World!"));
    ///         res
    ///     });
    /// });
    /// #     })
    /// #     .run("0.0.0.0:3000");
    /// ```
    pub fn path<S: AsRef<str>, H: Fn(&mut RoutePathBuilder) + Send + Sync + 'static>(
        &mut self,
        pattern: S,
        handler: H,
    ) -> &Self {
        let pattern = normalize_pattern(pattern.as_ref());
        let pattern = Regex::new(&pattern).expect("Pattern does not contain valid regex");

        let mut builder = RoutePathBuilder {
            pattern: pattern,
            routes: HashMap::new(),
        };

        handler(&mut builder);

        let _ = &self.routes.extend(builder.finish());

        self
    }
}

/// Route Path builder.
///
/// This is not to be used directly,
/// its only used for `RouteBuilder.path`.
pub struct RoutePathBuilder {
    pattern: Regex,
    routes: HashMap<Method, Vec<Route>>,
}

impl RoutePathBuilder {
    fn route<H: Fn(&Request, &State, &Captures) -> Response + Send + Sync + 'static>(
        &mut self,
        method: Method,
        handler: H,
    ) -> &Self {
        let handler = Box::new(handler);
        self.routes.entry(method).or_insert(Vec::new()).push(Route {
            handler,
            pattern: self.pattern.clone(),
        });
        self
    }

    fn finish(self) -> HashMap<Method, Vec<Route>> {
        self.routes
    }

    /// Adds a `Method::GET` request handler.
    ///
    /// # Examples
    /// ```
    /// # Direkuta::new()
    /// #     .route(|r| {
    /// r.path("/", |r| {
    ///     r.get(|_, _, _| {
    ///         let mut res = Response::new();
    ///         res.set_body(String::from("Hello World!"));
    ///         res
    ///     });
    /// });
    /// #     })
    /// #     .run("0.0.0.0:3000");
    /// ```
    pub fn get<H: Fn(&Request, &State, &Captures) -> Response + Send + Sync + 'static>(
        &mut self,
        handler: H,
    ) -> &Self {
        self.route(Method::GET, handler)
    }

    /// Adds a `Method::POST` request handler.
    ///
    /// # Examples
    /// ```
    /// # Direkuta::new()
    /// #     .route(|r| {
    /// r.path("/", |r| {
    ///     r.post(|_, _, _| {
    ///         let mut res = Response::new();
    ///         res.set_body(String::from("Hello World!"));
    ///         res
    ///     });
    /// });
    /// #     })
    /// #     .run("0.0.0.0:3000");
    /// ```
    pub fn post<H: Fn(&Request, &State, &Captures) -> Response + Send + Sync + 'static>(
        &mut self,
        handler: H,
    ) -> &Self {
        self.route(Method::POST, handler)
    }

    /// Adds a `Method::PUT` request handler.
    ///
    /// # Examples
    /// ```
    /// # Direkuta::new()
    /// #     .route(|r| {
    /// r.path("/", |r| {
    ///     r.put(|_, _, _| {
    ///         let mut res = Response::new();
    ///         res.set_body(String::from("Hello World!"));
    ///         res
    ///     });
    /// });
    /// #     })
    /// #     .run("0.0.0.0:3000");
    /// ```
    pub fn put<H: Fn(&Request, &State, &Captures) -> Response + Send + Sync + 'static>(
        &mut self,
        handler: H,
    ) -> &Self {
        self.route(Method::PUT, handler)
    }

    /// Adds a `Method::DELETE` request handler.
    ///
    /// # Examples
    /// ```
    /// # Direkuta::new()
    /// #     .route(|r| {
    /// r.path("/", |r| {
    ///     r.delete(|_, _, _| {
    ///         let mut res = Response::new();
    ///         res.set_body(String::from("Hello World!"));
    ///         res
    ///     });
    /// });
    /// #     })
    /// #     .run("0.0.0.0:3000");
    /// ```
    pub fn delete<H: Fn(&Request, &State, &Captures) -> Response + Send + Sync + 'static>(
        &mut self,
        handler: H,
    ) -> &Self {
        self.route(Method::DELETE, handler)
    }

    /// Adds a `Method::HEAD` request handler.
    ///
    /// # Examples
    /// ```
    /// # Direkuta::new()
    /// #     .route(|r| {
    /// r.path("/", |r| {
    ///     r.head(|_, _, _| {
    ///         let mut res = Response::new();
    ///         res.set_body(String::from("Hello World!"));
    ///         res
    ///     });
    /// });
    /// #     })
    /// #     .run("0.0.0.0:3000");
    /// ```
    pub fn head<H: Fn(&Request, &State, &Captures) -> Response + Send + Sync + 'static>(
        &mut self,
        handler: H,
    ) -> &Self {
        self.route(Method::HEAD, handler)
    }

    /// Adds a `Method::OPTIONS` request handler.
    ///
    /// # Examples
    /// ```
    /// # Direkuta::new()
    /// #     .route(|r| {
    /// r.path("/", |r| {
    ///     r.options(|_, _, _| {
    ///         let mut res = Response::new();
    ///         res.set_body(String::from("Hello World!"));
    ///         res
    ///     });
    /// });
    /// #     })
    /// #     .run("0.0.0.0:3000");
    /// ```
    pub fn options<H: Fn(&Request, &State, &Captures) -> Response + Send + Sync + 'static>(
        &mut self,
        handler: H,
    ) -> &Self {
        self.route(Method::OPTIONS, handler)
    }
}

/// A type wrapper for ease of use Captures
type Captures = Vec<(Option<String>, String)>;

struct RouteRecognizer {
    routes: HashMap<Method, Vec<Route>>,
}

impl RouteRecognizer {
    fn recognize(
        &self,
        method: &Method,
        path: &str,
    ) -> Result<
        (
            &(Fn(&Request, &State, &Captures) -> Response + Send + Sync + 'static),
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

/// A wrapper around Hyper Response.
#[derive(Debug)]
pub struct Response {
    pub(crate) body: Body,
    pub(crate) parts: response::Parts,
}

impl Response {
    /// Constructs a new `Response`
    ///
    /// # Examples
    /// ```
    /// let res = Response::new();
    /// ```
    pub fn new() -> Self {
        Response::default()
    }

    /// Return Response HTTP version.
    pub fn version(&self) -> Version {
        self.parts.version
    }

    /// Return Response HTTP headers.
    pub fn headers(&self) -> &HeaderMap<HeaderValue> {
        &self.parts.headers
    }

    /// Return Response HTTP headers.
    pub fn headers_mut(&mut self) -> &mut HeaderMap<HeaderValue> {
        &mut self.parts.headers
    }

    /// Set Response's HTTP headers.
    pub fn set_headers(&mut self, headers: HeaderMap<HeaderValue>) {
        self.parts.headers = headers;
    }

    /// Return Response HTTP status code.
    pub fn status(&self) -> StatusCode {
        self.parts.status
    }

    /// Get mutable reference to Response's status code.
    pub fn status_mut(&mut self) -> &mut StatusCode {
        &mut self.parts.status
    }

    /// Set Response's HTTP status code.
    pub fn set_status(&mut self, status: u16) {
        self.parts.status =
            StatusCode::from_u16(status).expect("Given status is not a valid status code");
    }

    /// Set Response's HTTP status code.
    pub fn with_status(mut self, status: u16) -> Self {
        self.set_status(status);
        self
    }

    /// Return Response HTTP body.
    pub fn body(self) -> Body {
        self.body
    }

    /// Get mutable reference to Response's body.
    pub fn body_mut(&mut self) -> &mut Body {
        &mut self.body
    }

    /// Set Response's HTTP body.
    pub fn set_body<T: Into<String>>(&mut self, body: T) {
        let body = body.into();
        let _ = self.headers_mut().insert(
            header::CONTENT_LENGTH,
            HeaderValue::from_str(&body.len().to_string())
                .expect("Given value for CONTENT_LENGTH is not valid"),
        );
        self.body = Body::from(body);
    }

    /// Set Response's HTTP body.
    pub fn with_body<T: Into<String>>(mut self, body: T) -> Self {
        self.set_body(body);
        self
    }

    /// Set Response's redirect location as status code.
    pub fn redirect(mut self, url: &'static str) -> Response {
        self.set_status(301);
        let _ = self
            .headers_mut()
            .insert(header::LOCATION, HeaderValue::from_static(url));

        self
    }

    /// Wrapper around `Request.set_body` for the HTML context type.
    pub fn html(&mut self, html: &str) {
        let _ = self
            .headers_mut()
            .insert(header::CONTENT_TYPE, HeaderValue::from_static("text/html"));

        self.set_body(html);
    }

    /// Wrapper around `Request.set_body` for the JS context type.
    pub fn js(&mut self, js: &str) {
        let _ = self.headers_mut().insert(
            header::CONTENT_TYPE,
            HeaderValue::from_static("application/javascript"),
        );

        self.set_body(js);
    }

    /// Wrapper around `Request.set_body` for the JSON context type.
    pub fn json<J: Serialize + Send + Sync>(&mut self, json: J) {
        let _ = self.headers_mut().insert(
            header::CONTENT_TYPE,
            HeaderValue::from_static("application/json"),
        );

        let mut wrapper: Wrapper<J> = Wrapper::new();
        wrapper.set_code(self.parts.status.as_u16());
        wrapper.set_status(&self.parts.status.as_str());
        wrapper.set_result(json);

        let json = serde_json::to_string(&wrapper).expect("Can not transform strcut into json");
        self.set_body(json);
    }

    /// An error happened, add a error message to be send out instead.
    pub fn json_error(&mut self, messages: Vec<&str>) {
        let _ = self.headers_mut().insert(
            header::CONTENT_TYPE,
            HeaderValue::from_static("application/json"),
        );

        let mut wrapper: Wrapper<()> = Wrapper::new();
        wrapper.set_code(self.parts.status.as_u16());
        wrapper.set_status(&self.parts.status.as_str());

        for message in messages {
            wrapper.add_message(message);
        }

        let json = serde_json::to_string(&wrapper).expect("Can not transform strcut into json");
        self.set_body(json);
    }

    /// Transform the Response intot a Hyper Response.
    pub fn into_hyper(self) -> hyper::Response<Body> {
        hyper::Response::from_parts(self.parts, self.body)
    }
}

impl Default for Response {
    fn default() -> Response {
        let (parts, body) = hyper::Response::new(Body::empty()).into_parts();
        Response { body, parts }
    }
}

/// A wrapped Hyper request.
#[derive(Debug)]
pub struct Request {
    pub(crate) body: Body,
    pub(crate) parts: request::Parts,
}

impl Request {
    /// Constructs a new `Request`
    pub fn new(body: Body, parts: request::Parts) -> Self {
        Self { body, parts }
    }

    /// Return Request HTTP version.
    pub fn version(&self) -> Version {
        self.parts.version
    }

    /// Return Request HTTP heads.
    pub fn headers(&self) -> &HeaderMap<HeaderValue> {
        &self.parts.headers
    }

    /// Return Request HTTP method.
    pub fn method(&self) -> &Method {
        &self.parts.method
    }

    /// Return Request uri.
    pub fn uri(&self) -> &Uri {
        &self.parts.uri
    }

    /// Return Request uri path.
    pub fn path(&self) -> &str {
        self.parts.uri.path()
    }

    /// Return Request body.
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
    fn new() -> Wrapper<T> {
        Wrapper::default()
    }

    fn add_message(&mut self, message: &str) {
        self.messages.push(String::from(message));
    }

    fn set_code(&mut self, code: u16) {
        self.code = code;
    }

    fn set_status(&mut self, status: &str) {
        self.status = String::from(status);
    }

    fn set_result(&mut self, result: T) {
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
