//! A web framework built around Hyper.
//!
//! # Examples
//!
//! ```rust,ignore
//! # use direkuta::prelude::*;
//! // Not tested due to the fact that its a web server.
//! Direkuta::new()
//!     .route(|r| {
//!         r.get("/", |_, _, _| {
//!             Response::new().with_body("Hello World!")
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
extern crate indexmap;
extern crate regex;
extern crate smallvec;

#[cfg(feature = "json")]
extern crate serde;
#[cfg(feature = "json")]
#[macro_use]
extern crate serde_derive;
#[cfg(feature = "json")]
extern crate serde_json;

#[cfg(feature = "html")]
extern crate tera;

use std::any::{Any, TypeId};
use std::borrow::Cow;
use std::fs::File;
use std::io::prelude::*;
use std::sync::Arc;

use futures::{future, Future};
use http::{request, response};
use hyper::header::{self, HeaderMap, HeaderValue};
use hyper::service::{NewService, Service};
use hyper::{rt, Body, Method, Server, StatusCode, Uri, Version};
use indexmap::IndexMap;
use regex::Regex;
use smallvec::SmallVec;

#[cfg(feature = "json")]
use serde::Serialize;

#[cfg(feature = "html")]
use tera::Tera;

/// The Direkuta web server itself.
pub struct Direkuta {
    /// Store state as its own type.
    state: Arc<State>,
    /// Stores middleware, to be later used in [Service::call](Service::call).
    middle: Arc<IndexMap<TypeId, Box<Middle + Send + Sync + 'static>>>,
    /// The router, it knows where a url is meant to go.
    routes: Arc<Router>,
}

impl Direkuta {
    /// Constructs a new [Direkuta](Direkuta).
    ///
    /// # Examples
    ///
    /// ```rust
    /// # use direkuta::prelude::*;
    /// Direkuta::new();
    /// ```
    pub fn new() -> Self {
        Direkuta::default()
    }

    /// Insert a state into [Direkuta](Direkuta).
    ///
    /// # Examples
    ///
    /// ```rust
    /// # use direkuta::prelude::*;
    /// Direkuta::new()
    ///     .state(String::from("Hello World!"));
    /// ```
    ///
    /// # Panics
    /// Do not use this from anywhere else but the main constructor.
    /// Using this from any else will cause a thread panic.
    pub fn state<T: Any + Send + Sync + 'static>(mut self, state: T) -> Self {
        Arc::get_mut(&mut self.state)
            .expect("Cannot get_mut on state")
            .set(state);
        self
    }

    /// Insert a middleware into [Direkuta](Direkuta).
    ///
    /// Middleware is anything that impliments the trait [Middle](Middle).
    ///
    /// # Examples
    ///
    /// ```rust
    /// # use direkuta::prelude::*;
    /// Direkuta::new()
    ///     .middle(Logger::new());
    /// ```
    ///
    /// # Panics
    ///
    /// Do not use this from anywhere else but the main constructor.
    /// Using this from any else will cause a thread panic.
    pub fn middle<T: Middle + Send + Sync + 'static>(mut self, middle: T) -> Self {
        let _ = Arc::get_mut(&mut self.middle)
            .expect("Cannot get_mut on middle")
            .insert(TypeId::of::<T>(), Box::new(middle));
        self
    }

    /// Create new router as a closure.
    ///
    /// # Examples
    ///
    /// ```rust
    /// # use direkuta::prelude::*;
    /// Direkuta::new()
    ///     .route(|r| {
    ///         // handlers here
    ///     });
    /// ```
    pub fn route<R: Fn(&mut Router) + Send + Sync + 'static>(mut self, route: R) -> Self {
        let mut route_builder = Router::new();

        route(&mut route_builder);
        self.routes = Arc::new(route_builder);

        self
    }

    /// Run [Direkuta](Direkuta) as a Hyper server.
    ///
    /// # Examples
    ///
    /// ```rust,ignore
    /// # use direkuta::prelude::*;
    /// // Not tested due to the fact that its a web server.
    /// Direkuta::new()
    ///     .run("0.0.0.0:3000");
    /// ```
    ///
    /// # Errors
    ///
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
        #[allow(unused_mut)]
        let mut state = State::new();

        #[cfg(feature = "html")]
        state.set(match Tera::parse("templates/**/*") {
            Ok(t) => t,
            Err(e) => {
                println!("Parsing error(s): {}", e);
                ::std::process::exit(1);
            }
        });

        Self {
            state: Arc::new(state),
            middle: Arc::new(IndexMap::new()),
            routes: Arc::new(Router::default()),
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
        let mut req = Request::new(body, parts);

        for (_, before) in self.middle.iter() {
            before.before(&mut req);
        }

        let mut res: Response = match self.routes.recognize(&method, &path) {
            Ok((handler, cap)) => handler(&req, &self.state.clone(), &cap),
            Err(code) => {
                let mut res = Response::new();
                res.set_status(code.as_u16());
                res
            }
        };

        for (_, after) in self.middle.iter() {
            after.after(&mut req, &mut res);
        }

        Box::new(future::ok(res.into_hyper()))
    }
}

/// All middleware must implement this trait.
///
/// # Examples
///
/// ```rust
/// # use direkuta::prelude::{Middle, Request, Response};
/// struct Logger {}
///
/// impl Logger {
///     pub fn new() -> Self {
///         Self { }
///     }
/// }
///
/// impl Middle for Logger {
///     fn before(&self, req: &mut Request) {
///         println!("[{}] `{}`", req.method(), req.uri());
///     }
///
///     fn after(&self, req: &mut Request, res: &mut Response) {
///         println!("[{}] `{}`", res.status(), req.uri());
///     }
/// }
/// ```
pub trait Middle {
    /// Called before a request is sent through [RouteRecognizer](RouteRecognizer)
    fn before(&self, &mut Request);
    /// Called after a request is sent through [RouteRecognizer](RouteRecognizer)
    fn after(&self, &mut Request, &mut Response);
}

/// A simple logger middleware.
///
/// # Examples
///
/// ```rust
/// # use direkuta::prelude::*;
/// Direkuta::new()
///     .middle(Logger::new());
/// ```
#[derive(Clone, Copy, Debug)]
pub struct Logger {}

impl Logger {
    /// Constructs a new [Logger](Logger).
    pub fn new() -> Self {
        Logger::default()
    }
}

impl Middle for Logger {
    fn before(&self, req: &mut Request) {
        println!("[{}] `{}`", req.method(), req.uri());
    }

    fn after(&self, req: &mut Request, res: &mut Response) {
        println!("[{}] `{}`", res.status(), req.uri());
    }
}

impl Default for Logger {
    fn default() -> Logger {
        Logger {}
    }
}

/// A wrapper around [HashMap](std::collections::HashMap)<[TypeId](std::any::TypeId), [Any](std::any::Any)>, used to store [Direkuta](Direkuta) state.
///
/// Stored state cannot be dynamically create and must be static.
pub struct State {
    inner: IndexMap<TypeId, Box<Any + Send + Sync + 'static>>,
}

impl State {
    /// Constructs a new [State](State)
    ///
    /// # Examples
    ///
    /// ```rust
    /// # use direkuta::prelude::*;
    /// let state = State::new();
    /// ```
    pub fn new() -> Self {
        State::default()
    }

    /// Sets the value of whatever type is passed.
    ///
    /// Please note that you cannot have two states of the same types, one will overwrite the other.
    ///
    /// # Examples
    ///
    /// ```rust
    /// # use direkuta::State;
    /// let mut state = State::new();
    ///
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
    ///
    /// ```rust
    /// # use direkuta::prelude::*;
    /// let mut state = State::new();
    ///
    /// state.set(String::from("Hello World!"));
    ///
    /// match state.try_get::<String>() {
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
    /// This is a wrapper around [try_get](State::try_get).
    ///
    /// # Examples
    ///
    /// ```rust
    /// # use direkuta::prelude::*;
    /// let mut state = State::new();
    ///
    /// state.set(String::from("Hello World!"));
    ///
    /// println!("{}", state.get::<String>());
    /// ```
    ///
    /// # Panics
    ///
    /// If the key does not exist the function will panic
    ///
    /// If you do not know if the type exists use `try_get`.
    pub fn get<T: Any + Send + Sync + 'static>(&self) -> &T {
        self.try_get::<T>()
            .unwrap_or_else(|| panic!("Key not found in state: {:?}", &TypeId::of::<T>()))
    }
}

impl Default for State {
    fn default() -> State {
        State {
            inner: IndexMap::new(),
        }
    }
}

type Handler = Fn(&Request, &State, &IndexMap<String, String>) -> Response + Send + Sync + 'static;

enum Mode {
    Id,
    Regex,
    Look,
}

/// Router.
///
/// This is not to be used directly, it is only used for [Direkuta.route](Direkuta::route).
struct Route {
    handler: Box<Handler>,
    ids: SmallVec<[String; 64]>,
    path: String,
    pattern: Regex,
}

/// Router.
///
/// This is not to be used directly, it is only used for [Direkuta.route](Direkuta::route).
///
/// All examples for routing are shown with 'output' or what the paths will look like
/// and what the response would look like when called.
///
/// The format is as shown.
///
/// ```
/// URL : { Parameter => Capture } {
///     Method => Response
/// }
/// ```
pub struct Router {
    inner: IndexMap<Method, SmallVec<[Route; 128]>>,
}

impl Router {
    fn new() -> Router {
        Router::default()
    }

    /// Adds route to routing map.
    ///
    /// Its easier to the the helper functions.
    ///
    /// # Examples
    ///
    /// ## Simple
    ///
    /// ```rust
    /// # use direkuta::prelude::*;
    /// # use direkuta::prelude::hyper::*;
    /// Direkuta::new()
    ///     .route(|r| {
    ///         r.route(Method::GET, "/", |_, _, _| {
    ///             Response::new().with_body("Hello World!")
    ///         });
    ///     });
    /// ```
    ///
    /// ```
    /// "/" {
    ///     GET => "Hello World!"
    /// }
    /// ```
    ///
    /// ## Regex
    ///
    /// ```rust
    /// # use direkuta::prelude::*;
    /// # use direkuta::prelude::hyper::*;
    /// Direkuta::new()
    ///     .route(|r| {
    ///         r.route(Method::GET, "/<name:(.*)>", |_, _, c| {
    ///             Response::new().with_body(c.get("name").unwrap().as_str())
    ///         });
    ///     });
    /// ```
    ///
    /// ```
    /// "/txuritan" : { "name" => "txuritan" } {
    ///     GET => "txuritan"
    /// }
    /// ```
    pub fn route<
        S: Into<String>,
        H: Fn(&Request, &State, &IndexMap<String, String>) -> Response + Send + Sync + 'static,
    >(
        &mut self,
        method: Method,
        path: S,
        handler: H,
    ) {
        let path = path.into();

        // Transform the path in to ids and regex
        let reader = self.read(&path);

        self.inner
            .entry(method)
            .or_insert(SmallVec::new())
            .push(Route {
                handler: Box::new(handler),
                ids: reader.0,
                path: path,
                pattern: reader.1,
            });
    }

    /// Adds a [GET](Method::GET) request handler.
    ///
    /// # Examples
    ///
    /// ## Simple
    ///
    /// ```rust
    /// # use direkuta::prelude::*;
    /// Direkuta::new()
    ///     .route(|r| {
    ///         r.get("/", |_, _, _| {
    ///             Response::new().with_body("Hello World!")
    ///         });
    ///     });
    /// ```
    ///
    /// ## Regex
    ///
    /// ```rust
    /// # use direkuta::prelude::*;
    /// # use direkuta::prelude::hyper::*;
    /// Direkuta::new()
    ///     .route(|r| {
    ///         r.route(Method::GET, "/<name:(.*)>", |_, _, c| {
    ///             Response::new().with_body(c.get("name").unwrap().as_str())
    ///         });
    ///     });
    /// ```
    ///
    /// ```
    /// "/txuritan" : { "name" => "txuritan" } {
    ///     GET => "txuritan"
    /// }
    /// ```
    pub fn get<
        S: Into<String>,
        H: Fn(&Request, &State, &IndexMap<String, String>) -> Response + Send + Sync + 'static,
    >(
        &mut self,
        path: S,
        handler: H,
    ) {
        self.route(Method::GET, path, handler);
    }

    /// Adds a [POST](Method::POST) request handler.
    ///
    /// # Examples
    ///
    /// ## Simple
    ///
    /// ```rust
    /// # use direkuta::prelude::*;
    /// Direkuta::new()
    ///     .route(|r| {
    ///         r.post("/", |_, _, _| {
    ///             Response::new().with_body("Hello World!")
    ///         });
    ///     });
    /// ```
    ///
    /// ```
    /// "/" : {  } {
    ///     POST => "Hello World!"
    /// }
    /// ```
    pub fn post<
        S: Into<String>,
        H: Fn(&Request, &State, &IndexMap<String, String>) -> Response + Send + Sync + 'static,
    >(
        &mut self,
        path: S,
        handler: H,
    ) {
        self.route(Method::POST, path, handler);
    }

    /// Adds a [PUT](Method::PUT) request handler.
    ///
    /// # Examples
    ///
    /// ## Simple
    ///
    /// ```rust
    /// # use direkuta::prelude::*;
    /// Direkuta::new()
    ///     .route(|r| {
    ///         r.put("/", |_, _, _| {
    ///             Response::new().with_body("Hello World!")
    ///         });
    ///     });
    /// ```
    ///
    /// ```
    /// "/" : {  } {
    ///     PUT => "Hello World!"
    /// }
    /// ```
    pub fn put<
        S: Into<String>,
        H: Fn(&Request, &State, &IndexMap<String, String>) -> Response + Send + Sync + 'static,
    >(
        &mut self,
        path: S,
        handler: H,
    ) {
        self.route(Method::PUT, path, handler);
    }

    /// Adds a [DELETE](Method::DELETE) request handler.
    ///
    /// # Examples
    ///
    /// ## Simple
    ///
    /// ```rust
    /// # use direkuta::prelude::*;
    /// Direkuta::new()
    ///     .route(|r| {
    ///         r.delete("/", |_, _, _| {
    ///             Response::new().with_body("Hello World!")
    ///         });
    ///     });
    /// ```
    ///
    /// ```
    /// "/" : {  } {
    ///     DELETE => "Hello World!"
    /// }
    /// ```
    pub fn delete<
        S: Into<String>,
        H: Fn(&Request, &State, &IndexMap<String, String>) -> Response + Send + Sync + 'static,
    >(
        &mut self,
        path: S,
        handler: H,
    ) {
        self.route(Method::DELETE, path, handler);
    }

    /// Adds a [HEAD](Method::HEAD) request handler.
    ///
    /// # Examples
    ///
    /// ## Simple
    ///
    /// ```rust
    /// # use direkuta::prelude::*;
    /// Direkuta::new()
    ///     .route(|r| {
    ///         r.head("/", |_, _, _| {
    ///             Response::new().with_body("Hello World!")
    ///         });
    ///     });
    /// ```
    ///
    /// ```
    /// "/" : {  } {
    ///     HEAD => "Hello World!"
    /// }
    /// ```
    pub fn head<
        S: Into<String>,
        H: Fn(&Request, &State, &IndexMap<String, String>) -> Response + Send + Sync + 'static,
    >(
        &mut self,
        path: S,
        handler: H,
    ) {
        self.route(Method::HEAD, path, handler);
    }

    /// Adds a [OPTIONS](Method::OPTIONS) request handler.
    ///
    /// # Examples
    ///
    /// ## Simple
    ///
    /// ```rust
    /// # use direkuta::prelude::*;
    /// Direkuta::new()
    ///     .route(|r| {
    ///         r.options("/", |_, _, _| {
    ///             Response::new().with_body("Hello World!")
    ///         });
    ///     });
    /// ```
    ///
    /// ```
    /// "/" : {  } {
    ///     OPTIONS => "Hello World!"
    /// }
    /// ```
    pub fn options<
        S: Into<String>,
        H: Fn(&Request, &State, &IndexMap<String, String>) -> Response + Send + Sync + 'static,
    >(
        &mut self,
        path: S,
        handler: H,
    ) {
        self.route(Method::OPTIONS, path, handler);
    }

    /// Create a path for multiple request types.
    ///
    /// # Examples
    ///
    /// ## Simple
    ///
    /// ```rust
    /// # use direkuta::prelude::*;
    /// Direkuta::new()
    ///     .route(|r| {
    ///         r.path("/parent", |r| {
    ///             r.get("/child", |_, _, _| {
    ///                 Response::new().with_body("Hello World!")
    ///             });
    ///         });
    ///     });
    /// ```
    ///
    /// ```
    /// "/parent/child" : {  } {
    ///     GET => "Hello World!"
    /// }
    /// ```
    pub fn path<
        S: Into<String>,
        F: Fn(&mut Router) + Send + Sync + 'static
    >(
        &mut self,
        path: S,
        sub: F
    ) {
        let mut builder = Router::new();

        sub(&mut builder);

        let path = path.into();

        // Loop through new methods
        for (method, routes) in builder.inner {
            // Loop through new routes
            for route in routes {
                // Concat paths
                let npath = format!("{}{}", path, route.path);

                // Transform the path in to ids and regex
                let reader = self.read(&npath);

                self.inner
                    .entry(method.clone())
                    .or_insert(SmallVec::new())
                    .push(Route {
                        handler: route.handler,
                        ids: reader.0,
                        path: npath,
                        pattern: reader.1,
                    });
            }
        }
    }

    /// When a request is recived this is called to find a handler.
    fn recognize(
        &self,
        method: &Method,
        path: &str,
    ) -> Result<(&Handler, IndexMap<String, String>), StatusCode> {
        // Get method
        let routes = self.inner.get(method).ok_or(StatusCode::NOT_FOUND)?;

        // Loop through all routes of method
        for route in routes.iter() {
            // Make sure the route matches
            if route.pattern.is_match(path) {
                // Get the capture map
                if let Some(map) = self.captures(&route, &route.pattern, path) {
                    return Ok((&*route.handler, map));
                }
            }
        }

        Err(StatusCode::NOT_FOUND)
    }

    /// Takes each capture and transfroms it into a map of ids and captures.
    fn captures(&self, route: &Route, re: &Regex, path: &str) -> Option<IndexMap<String, String>> {
        // Get captures
        re.captures(path).map(|caps| {
            let mut res: IndexMap<String, String> = IndexMap::new();

            // Loop through each capture
            for (i, _) in caps.iter().enumerate() {
                // We dont want the frist whole capture
                if i != 0 {
                    // Insert the capture to its id
                    let _ = res.insert(
                        // An id exists so the unwrap is safe
                        route.ids.get(i - 1).unwrap().to_string(),
                        // The capture exists so the unwrap is safe
                        caps.get(i).unwrap().as_str().to_string(),
                    );
                }
            }

            res
        })
    }

    /// Parse each path into a vector of ids and a regex pattern
    fn read(&self, path: &str) -> (SmallVec<[String; 64]>, Regex) {
        let mut ids: SmallVec<[String; 64]> = SmallVec::new();
        let mut pattern = String::new();

        let mut mode = Mode::Look;
        let mut id = String::new();

        for c in path.chars() {
            match c {
                '<' => mode = Mode::Id,
                ':' => {
                    mode = Mode::Regex;
                    ids.push(id.clone());
                    id.clear();
                }
                '>' => mode = Mode::Look,
                _ => match mode {
                    Mode::Id => id.push(c),
                    Mode::Regex | Mode::Look => pattern.push(c),
                },
            }
        }

        (ids, Regex::new(&self.normalize(&pattern)).unwrap())
    }

    /// Normalizes the regex paths.
    ///
    /// Removes the beginning `^` and ending `$` and `/`, if the exist.
    /// Then adds them even if they weren't there.
    fn normalize(&self, pattern: &str) -> Cow<str> {
        let pattern = pattern
            .trim()
            .trim_left_matches('^')
            .trim_right_matches('$')
            .trim_right_matches('/');
        match pattern {
            "" => "^/$".into(),
            s => format!("^{}/?$", s).into(),
        }
    }
}

impl Default for Router {
    fn default() -> Router {
        Router {
            inner: IndexMap::new(),
        }
    }
}

/// A wrapper around [Hyper Response](hyper::Response).
#[derive(Debug)]
pub struct Response {
    body: Body,
    parts: response::Parts,
}

impl Response {
    /// Constructs a new `Response`.
    ///
    /// # Examples
    ///
    /// ```rust
    /// # use direkuta::prelude::*;
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
    ///
    /// # Examples
    ///
    /// ```rust
    /// # use direkuta::prelude::*;
    /// # use direkuta::prelude::hyper::*;
    /// let mut res = Response::new();
    /// res.headers_mut().insert(
    ///     header::CONTENT_TYPE,
    ///     HeaderValue::from_static("text/plain")
    /// );
    /// ```
    pub fn headers_mut(&mut self) -> &mut HeaderMap<HeaderValue> {
        &mut self.parts.headers
    }

    /// Set Response's HTTP headers.
    pub fn set_headers(&mut self, headers: HeaderMap<HeaderValue>) {
        self.parts.headers.extend(headers);
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
    ///
    /// # Examples
    ///
    /// ```rust
    /// # use direkuta::prelude::*;
    /// let mut res = Response::new();
    /// res.set_status(404);
    /// ```
    pub fn set_status(&mut self, status: u16) {
        self.parts.status =
            StatusCode::from_u16(status).expect("Given status is not a valid status code");
    }

    /// Set Response's HTTP status code.
    ///
    /// # Examples
    ///
    /// ```rust
    /// # use direkuta::prelude::*;
    /// let res = Response::new()
    ///     .with_status(404);
    /// ```
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
    ///
    /// # Examples
    ///
    /// ```rust
    /// # use direkuta::prelude::*;
    /// let mut res = Response::new();
    /// res.set_body("Hello World!");
    /// ```
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
    ///
    /// # Examples
    ///
    /// ```rust
    /// # use direkuta::prelude::*;
    /// let res = Response::new()
    ///     .with_body("Hello World!");
    /// ```
    pub fn with_body<T: Into<String>>(mut self, body: T) -> Self {
        self.set_body(body);
        self
    }

    /// Set Response's redirect location as status code.
    ///
    /// # Examples
    ///
    /// ```rust
    /// # use direkuta::prelude::*;
    /// let mut res = Response::new();
    /// res.redirect("/example/moved");
    /// ```
    pub fn redirect(&mut self, url: &'static str) {
        self.set_status(301);
        let _ = self
            .headers_mut()
            .insert(header::LOCATION, HeaderValue::from_static(url));
    }

    /// Set Response's redirect location as status code.
    ///
    /// # Examples
    ///
    /// ```rust
    /// # use direkuta::prelude::*;
    /// let res = Response::new()
    ///     .with_redirect("/example/moved");
    /// ```
    pub fn with_redirect(mut self, url: &'static str) -> Self {
        self.redirect(url);
        self
    }

    // TODO: Change this into a builder closure, with string, file, and template functions.
    /// Wrapper around [Response.set_body](Response::set_body) for the HTML context type.
    pub fn html<T: Into<String>>(&mut self, html: T) {
        let _ = self
            .headers_mut()
            .insert(header::CONTENT_TYPE, HeaderValue::from_static("text/html"));

        self.set_body(html);
    }

    /// Wrapper around [Response.set_body](Response::set_body) for the CSS context type.
    ///
    /// # Examples
    ///
    /// ```rust
    /// # use direkuta::prelude::*;
    /// let mut res = Response::new();
    /// res.css(|c| {
    ///     c.path("/static/app.css");
    /// });
    /// ```
    pub fn css<F: Fn(&mut CssBuilder)>(&mut self, css: F) {
        let _ = self
            .headers_mut()
            .insert(header::CONTENT_TYPE, HeaderValue::from_static("text/css"));

        let mut builder = CssBuilder::new();

        css(&mut builder);

        self.set_body(builder.get_body());
    }

    /// Wrapper around [Response.set_body](Response::set_body) for the CSS context type.
    ///
    /// # Examples
    ///
    /// ```rust
    /// # use direkuta::prelude::*;
    /// let res = Response::new()
    ///     .with_css(|c| {
    ///         c.path("/static/app.css");
    ///     });
    /// ```
    pub fn with_css<F: Fn(&mut CssBuilder)>(mut self, css: F) -> Self {
        self.css(css);
        self
    }

    /// Wrapper around [Response.set_body](Response::set_body) for the JS context type.
    ///
    /// # Examples
    ///
    /// ```rust
    /// # use direkuta::prelude::*;
    /// let mut res = Response::new();
    /// res.js(|j| {
    ///     j.path("/static/app.js");
    /// });
    /// ```
    pub fn js<F: Fn(&mut JsBuilder)>(&mut self, js: F) {
        let _ = self.headers_mut().insert(
            header::CONTENT_TYPE,
            HeaderValue::from_static("application/javascript"),
        );

        let mut builder = JsBuilder::new();

        js(&mut builder);

        self.set_body(builder.get_body());
    }

    /// Wrapper around [Response.set_body](Response::set_body) for the JS context type.
    ///
    /// # Examples
    ///
    /// ```rust
    /// # use direkuta::prelude::*;
    /// let res = Response::new()
    ///     .with_js(|j| {
    ///         j.path("/static/app.js");
    ///     });
    /// ```
    pub fn with_js<F: Fn(&mut JsBuilder)>(mut self, js: F) -> Self {
        self.js(js);
        self
    }

    /// Wrapper around [Response.set_body](Response::set_body) for the JSON context type.
    ///
    /// # Examples
    ///
    /// ```rust
    /// # extern crate direkuta;
    /// # #[macro_use] extern crate serde_derive;
    ///
    /// use direkuta::prelude::*;
    ///
    /// #[derive(Serialize)]
    /// struct Example {
    ///     hello: String,
    /// }
    /// # fn main() {
    /// let mut res = Response::new();
    /// res.json(|j| {
    ///     j.body(Example {
    ///         hello: String::from("world"),
    ///     });
    /// });
    /// # }
    /// ```
    #[cfg(feature = "json")]
    pub fn json<T: Serialize + Send + Sync, F: Fn(&mut JsonBuilder<T>)>(&mut self, json: F) {
        let mut builder = JsonBuilder::new::<T>();

        let _ = self.headers_mut().insert(
            header::CONTENT_TYPE,
            HeaderValue::from_static("application/json"),
        );

        json(&mut builder);

        self.set_body(builder.get_body());
    }

    /// Builder function for Json responses
    ///
    /// # Examples
    ///
    /// ```rust
    /// # extern crate direkuta;
    /// # #[macro_use] extern crate serde_derive;
    ///
    /// use direkuta::prelude::*;
    ///
    /// #[derive(Serialize)]
    /// struct Example {
    ///     hello: String,
    /// }
    /// # fn main() {
    /// let res = Response::new()
    ///     .with_json(|j| {
    ///         j.body(Example {
    ///             hello: String::from("world"),
    ///         });
    ///     });
    /// # }
    /// ```
    #[cfg(feature = "json")]
    pub fn with_json<T: Serialize + Send + Sync, F: Fn(&mut JsonBuilder<T>)>(
        mut self,
        json: F,
    ) -> Self {
        self.json(json);
        self
    }

    /// Transform the Response into a Hyper Response.
    fn into_hyper(self) -> hyper::Response<Body> {
        hyper::Response::from_parts(self.parts, self.body)
    }
}

impl Default for Response {
    fn default() -> Response {
        let (parts, body) = hyper::Response::new(Body::empty()).into_parts();
        Response { body, parts }
    }
}

/// A builder function for CSS Responses.
///
/// Do not directly use.
pub struct CssBuilder {
    inner: String,
}

impl CssBuilder {
    fn new() -> CssBuilder {
        CssBuilder::default()
    }

    fn get_body(&self) -> &str {
        self.inner.as_str()
    }

    /// Load from [File](std::fs::File).
    pub fn file(&mut self, mut file: File) {
        match file.read_to_string(&mut self.inner) {
            Ok(_) => {}
            Err(_) => println!("Unable to write file contents"),
        }
    }
}

impl Default for CssBuilder {
    fn default() -> CssBuilder {
        CssBuilder {
            inner: String::new(),
        }
    }
}

/// A builder function for JS Responses.
///
/// Do not directly use.
pub struct JsBuilder {
    inner: String,
}

impl JsBuilder {
    fn new() -> JsBuilder {
        JsBuilder::default()
    }

    fn get_body(&self) -> &str {
        self.inner.as_str()
    }

    /// Load from [File](std::fs::File).
    pub fn file(&mut self, mut file: File) {
        match file.read_to_string(&mut self.inner) {
            Ok(_) => {}
            Err(_) => println!("Unable to write file contents"),
        }
    }
}

impl Default for JsBuilder {
    fn default() -> JsBuilder {
        JsBuilder {
            inner: String::new(),
        }
    }
}

/// A builder for JSON responses.
///
/// Do not directly use.
#[cfg(feature = "json")]
pub struct JsonBuilder<T: Serialize + Send + Sync> {
    /// Json response wrapper to be sent.
    wrapper: Wrapper<T>,
}

#[cfg(feature = "json")]
impl JsonBuilder<()> {
    /// Creates a [JsonBuilder](JsonBuilder) with given type.
    fn new<T: Serialize + Send + Sync>() -> JsonBuilder<T> {
        JsonBuilder::default()
    }
}

#[cfg(feature = "json")]
impl<T: Serialize + Send + Sync> JsonBuilder<T> {
    /// Set the body of the wrapper.
    pub fn body(&mut self, body: T) {
        self.wrapper.set_result(body);
    }

    /// Set the body of the wrapper.
    pub fn with_body(mut self, body: T) -> Self {
        self.body(body);
        self
    }

    /// Added an error message to the wrapper.
    pub fn error(&mut self, message: &str) {
        self.wrapper.add_message(message);
    }

    /// Added an error message to the wrapper.
    pub fn errors(&mut self, messages: Vec<&str>) {
        for message in messages {
            self.wrapper.add_message(message);
        }
    }

    /// Set the status code of the Json response.
    ///
    /// This can be gotten with [StatusCode.as_u16](StatusCode::as_u16).
    pub fn code(&mut self, status: u16) {
        self.wrapper.set_code(status);
    }

    /// Set the status code of the Json response.
    ///
    /// This can be gotten with [StatusCode.as_u16](StatusCode::as_u16).
    pub fn with_code(mut self, status: u16) -> Self {
        self.code(status);
        self
    }

    /// Set the status string of the Json response.
    ///
    /// This can be gotten with [StatusCode.as_str](StatusCode::as_str).
    pub fn status(&mut self, status: &str) {
        self.wrapper.set_status(status);
    }

    /// Set the status string of the Json response.
    ///
    /// This can be gotten with [StatusCode.as_str](StatusCode::as_str).
    pub fn with_status(mut self, status: &str) -> Self {
        self.status(status);
        self
    }

    fn get_body(&self) -> String {
        serde_json::to_string(&self.wrapper).expect("Can not transform strcut into json")
    }
}

#[cfg(feature = "json")]
impl<T: Serialize + Send + Sync> Default for JsonBuilder<T> {
    fn default() -> JsonBuilder<T> {
        Self {
            wrapper: Wrapper::new(),
        }
    }
}

#[cfg(feature = "json")]
#[derive(Serialize)]
struct Wrapper<T: Serialize + Send + Sync> {
    code: u16,
    messages: Vec<String>,
    result: Option<T>,
    status: String,
}

#[cfg(feature = "json")]
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

#[cfg(feature = "json")]
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

/// A wrapper around [Hyper Request](hyper::Request).
#[derive(Debug)]
pub struct Request {
    body: Body,
    parts: request::Parts,
}

impl Request {
    /// Constructs a new [Request](Request).
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
    pub fn body(&self) -> &Body {
        &self.body
    }
}

/// Creates a [HeaderMap](HeaderMap) from a list of key-value pairs.
///
/// # Examples
///
/// ```rust
/// #[macro_use]
/// extern crate direkuta;
///
/// use direkuta::prelude::*;
/// use direkuta::prelude::hyper::*;
///
/// # fn main() {
/// Direkuta::new()
///     .route(|r| {
///         r.get("/", |_, _, _| {
///             let mut res = Response::new().with_body("Hello World!");
///             res.set_headers(headermap! {
///                 header::CONTENT_TYPE => "text/plain",
///             });
///             res
///         });
///     });
/// # }
/// ```
#[macro_export]
macro_rules! headermap {
    (@single $($x:tt)*) => (());
    (@count $($rest:expr),*) => (<[()]>::len(&[$(headermap!(@single $rest)),*]));

    ($($key:expr => $value:expr,)+) => { headermap!($($key => $value),+) };
    ($($key:expr => $value:expr),*) => {
        {
            let _cap = headermap!(@count $($key),*);
            let mut _map = ::direkuta::prelude::hyper::HeaderMap::with_capacity(_cap);
            $(
                let _ = _map.insert($key, ::direkuta::prelude::hyper::HeaderValue::from_static($value));
            )*
            _map
        }
    };
}

/// Imports just the required parts of [Direkuta](Direkuta).
pub mod prelude {
    pub use super::{Direkuta, Logger, Middle, Request, Response, State};

    /// Imports the required parts from [Tera](Tera).
    ///
    /// You'll need to import this if you want to use Tera templates.
    #[cfg(feature = "html")]
    pub mod html {
        pub use tera::{Context, Tera};
    }

    /// Imports the required parts from [Hyper](Hyper).
    ///
    /// You'll need this if you want to create a handler that doesn't have a function
    /// or if you want to set response Headers.
    pub mod hyper {
        pub use hyper::header::{self, HeaderMap, HeaderValue};
        pub use hyper::Method;
    }
}
