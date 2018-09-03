//! A web framework built around Hyper.
//!
//! # Examples
//!
//! Please note that none of these are tested due to Direkuta being a web sever.
//!
//! If, for some reason, they don't work, have a look at the examples folder.
//!
//! ## Simple
//!
//! ```rust,ignore
//! use direkuta::prelude::*;
//!
//! Direkuta::new()
//!     .route(|r| {
//!         r.get("/", |_, _, _| {
//!             Response::new().with_body("Hello World!")
//!         });
//!     })
//!     .run("0.0.0.0:3000");
//! ```
//!
//! ## Tera Templates
//!
//! ```rust,ignore
//! extern crate direkuta;
//!
//! use direkuta::prelude::*;sp
//! use direkuta::prelude::html::*;
//!
//! fn main() {
//!     Direkuta::new()
//!         .route(|r| {
//!             r.get("/", |_, s, _| {
//!                 Response::new().with_body(s
//!                     .get::<Tera>()
//!                     .render(Context::new(), "index.html")
//!                     .unwrap())
//!             });
//!         }).run("0.0.0.0:3000");
//! }
//! ```
//!
//! ## JSON
//!
//! ```rust,ignore
//! extern crate direkuta;
//! #[macro_use]
//! extern crate serde_derive;
//!
//! use direkuta::prelude::*;
//!
//! #[derive(Serialize)]
//! struct Example {
//!     hello: String,
//! }
//!
//! fn main() {
//!     Direkuta::new()
//!         .route(|r| {
//!             r.get("/", |_, _, _| {
//!                 Response::new().with_json(|j| {
//!                     j.body(Example {
//!                         hello: String::from("world"),
//!                     });
//!                 })
//!             });
//!         }).run("0.0.0.0:3000");
//! }
//! ```
//!
//! ## Routing
//!
//! ```rust,ignore
//! extern crate direkuta;
//!
//! use direkuta::prelude::*;
//!
//! fn main() {
//!     Direkuta::new()
//!         .route(|r| {
//!             r.get("/<name:(.+)>", |_, _, c| {
//!                 Response::new().with_body(c.get("name"))
//!             });
//!         }).run("0.0.0.0:3000");
//! }
//! ```
//!

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
extern crate tokio_fs;

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

#[cfg(feature = "json")]
use serde::Serialize;

#[cfg(feature = "html")]
use tera::Tera;

/// The Direkuta web server itself.
pub struct Direkuta {
    config: Arc<Config>,
    /// Store state as its own type.
    state: Arc<State>,
    /// Stores middleware, to be later used in Service::call.
    middle: Arc<IndexMap<TypeId, Box<Middle + Send + Sync + 'static>>>,
    /// The router, it knows where a url is meant to go.
    routes: Arc<Router>,
}

impl Direkuta {
    /// Constructs a new Direkuta.
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

    /// Set the configuration of the server.
    pub fn config<R: Fn(&mut Config) + Send + Sync + 'static>(mut self, c: R) -> Self {
        let mut config = Config::new();

        c(&mut config);
        self.config = Arc::new(config);

        self
    }

    /// Insert a state into server.
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
    ///
    /// Do not use this from anywhere else but the main constructor.
    /// Using this from any else will cause a thread panic.
    #[inline]
    pub fn state<T: Any + Send + Sync + 'static>(mut self, state: T) -> Self {
        Arc::get_mut(&mut self.state)
            .expect("Cannot get_mut on state")
            .set(state);
        self
    }

    /// Insert a middleware into server.
    ///
    /// Middleware is anything that implements the trait Middle.
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
    #[inline]
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
    #[inline]
    pub fn route<R: Fn(&mut Router) + Send + Sync + 'static>(mut self, route: R) -> Self {
        let mut route_builder = Router::new();

        route(&mut route_builder);
        self.routes = Arc::new(route_builder);

        self
    }

    /// Run server as a Hyper server.
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
    #[inline]
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
            config: Arc::new(Config::new()),
            state: Arc::new(state),
            middle: Arc::new(IndexMap::new()),
            routes: Arc::new(Router::default()),
        }
    }
}

impl NewService for Direkuta {
    type ReqBody = Body;
    type ResBody = Body;
    type Error = DireError;
    type InitError = DireError;
    type Service = Direkuta;
    type Future = Box<Future<Item = Self::Service, Error = Self::InitError> + Send>;

    fn new_service(&self) -> Self::Future {
        Box::new(future::ok(Self {
            config: self.config.clone(),
            state: self.state.clone(),
            middle: self.middle.clone(),
            routes: self.routes.clone(),
        }))
    }
}

impl Service for Direkuta {
    type ReqBody = Body;
    type ResBody = Body;
    type Error = DireError;
    type Future = Box<Future<Item = response::Response<Self::ResBody>, Error = Self::Error> + Send>;

    fn call(&mut self, req: request::Request<Self::ReqBody>) -> Self::Future {
        let path = req.uri().path().to_owned();
        let (parts, body) = req.into_parts();
        let mut req = Request::new(body, parts);

        for (_, before) in self.middle.iter() {
            before.run(&mut req);
        }

        match self.routes.recognize(&req.method(), &path) {
            Ok((handler, cap)) => handler(req, self.state.clone(), cap),
            Err(code) => Response::new().with_status(code.as_u16()).build(),
        }
    }
}

/// Internal configuration for the server.
///
/// Allows finer control of preset variables like template and static path.
pub struct Config {
    template_path: String,
    static_path: String,
}

impl Config {
    fn new() -> Self {
        Self::default()
    }

    /// Set the path for templates, defaults to "templates".
    #[inline]
    pub fn template_path<S: Into<String>>(&mut self, path: S) {
        self.template_path = path.into();
    }

    /// Set the path for static files, defaults to "static".
    #[inline]
    pub fn static_path<S: Into<String>>(&mut self, path: S) {
        self.static_path = path.into();
    }
}

impl Default for Config {
    fn default() -> Self {
        Self {
            template_path: "templates".to_string(),
            static_path: "static".to_string(),
        }
    }
}

/// Wrapper around common and library error types.
///
/// You should not have to create your own error type.
#[derive(Debug)]
pub enum DireError {
    /// Any error that originates from Hyper.
    Hyper(hyper::Error),
    /// General error, for use when no error type exists.
    Other(String),
}

impl std::fmt::Display for DireError {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        match *self {
            DireError::Hyper(ref e) => write!(f, "(DireError [Hyper] {})", e),
            DireError::Other(ref e) => write!(f, "(DireError [Other] {})", e),
        }
    }
}

impl std::error::Error for DireError {
    fn description(&self) -> &str {
        match *self {
            DireError::Hyper(ref e) => e.description(),
            DireError::Other(ref e) => e,
        }
    }

    fn cause(&self) -> Option<&std::error::Error> {
        match *self {
            DireError::Hyper(ref e) => e.cause(),
            _ => None,
        }
    }
}

impl From<hyper::Error> for DireError {
    fn from(err: hyper::Error) -> DireError {
        DireError::Hyper(err)
    }
}

impl From<&'static str> for DireError {
    fn from(err: &str) -> DireError {
        DireError::Other(err.to_string())
    }
}

impl From<String> for DireError {
    fn from(err: String) -> DireError {
        DireError::Other(err)
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
///     fn run(&self, req: &mut Request) {
///         println!("[{}] `{}`", req.method(), req.uri());
///     }
/// }
/// ```
pub trait Middle {
    /// Called before a request is sent through Router.
    fn run(&self, &mut Request);
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
pub struct Logger {}

impl Logger {
    /// Constructs a new Logger.
    pub fn new() -> Self {
        Logger::default()
    }
}

impl Middle for Logger {
    #[inline]
    fn run(&self, req: &mut Request) {
        println!("[{:>6}] `{}`", req.method().as_ref(), req.uri());
    }
}

impl Default for Logger {
    fn default() -> Logger {
        Logger {}
    }
}

/// A wrapper around IndexMap<TypeId, Any>, used to store server state.
///
/// Stored state cannot be dynamically created and must be static.
pub struct State {
    inner: IndexMap<TypeId, Box<Any + Send + Sync + 'static>>,
}

impl State {
    /// Constructs a new State.
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
    /// This is a wrapper around try_get.
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
    /// If the key does not exist the function will panic.
    ///
    /// If you do not know if the type exists use `try_get`.
    pub fn get<T: Any + Send + Sync + 'static>(&self) -> &T {
        self.try_get::<T>()
            .unwrap_or_else(|| panic!("Key not found in state: {:?}", &TypeId::of::<T>()))
    }
}

impl Default for State {
    fn default() -> Self {
        Self {
            inner: IndexMap::new(),
        }
    }
}

/// The current mode of the router path parser.
enum Mode {
    /// Currently writing the id of the capture.
    Id,
    /// Currently writing the regex of the capture.
    Regex,
    /// Looking for another Id or push any other character.
    Look,
}

/// A wrapper around IndexMap<String, String>.
///
/// Stores the captures for a given request.
pub struct Capture {
    inner: IndexMap<String, String>,
}

impl Capture {
    /// Constructs a new Capture.
    ///
    /// # Examples
    ///
    /// ```rust
    /// # use direkuta::prelude::*;
    /// let capture = Capture::new();
    /// ```
    pub fn new() -> Self {
        Capture::default()
    }

    /// Sets the value of whatever key is passed.
    ///
    /// Please note that you cannot have two of the same keys, one will overwrite the other.
    ///
    /// # Examples
    ///
    /// ```rust
    /// # use direkuta::prelude::*;
    /// let mut capture = Capture::new();
    ///
    /// capture.set("message", "Hello World!");
    /// ```
    #[inline]
    pub fn set<K: Into<String>, V: Into<String>>(&mut self, key: K, value: V) {
        let _ = self.inner.insert(key.into(), value.into());
    }

    /// Attempt to get a value based on key.
    ///
    /// Use this if you are not sure if the key exists.
    ///
    /// # Examples
    ///
    /// ```rust
    /// # use direkuta::prelude::*;
    /// let mut capture = Capture::new();
    ///
    /// capture.set("message", "Hello World!");
    ///
    /// match capture.try_get("message") {
    ///     Some(s) => {
    ///         println!("{}", s);
    ///     },
    ///     None => {
    ///         println!("Key not found in capture");
    ///     },
    /// }
    /// ```
    pub fn try_get<S: Into<String>>(&self, key: S) -> Option<&String> {
        self.inner.get(&key.into())
    }

    /// Get a value based on key.
    ///
    /// This is a wrapper around try_get.
    ///
    /// # Examples
    ///
    /// ```rust
    /// # use direkuta::prelude::*;
    /// let mut capture = Capture::new();
    ///
    /// capture.set("message", "Hello World!");
    ///
    /// println!("{}", capture.get("message"));
    /// ```
    ///
    /// # Panics
    ///
    /// If the key does not exist the function will panic
    ///
    /// If you do not know if the key exists use `try_get`.
    pub fn get<S: Into<String>>(&self, key: S) -> &str {
        let key = key.into();
        self.try_get(key.as_str())
            .unwrap_or_else(|| panic!("Key not found in captures: {}", key))
    }
}

impl Default for Capture {
    fn default() -> Capture {
        Capture {
            inner: IndexMap::new(),
        }
    }
}

type Handler =
    Fn(Request, Arc<State>, Capture)
            -> Box<Future<Item = response::Response<Body>, Error = DireError> + Send + 'static>
        + Send
        + Sync
        + 'static;

/// Internal route, stores the handler and path details.
///
/// This is not to be used directly, it is only used for Direkuta.route.
struct Route {
    handler: Box<Handler>,
    ids: Vec<String>,
    path: String,
    pattern: Regex,
}

/// Router.
///
/// This is not to be used directly, it is only used for Direkuta.route.
///
/// All examples for routing are shown with 'output' or what the paths will look like
/// and what the response would look like when called.
///
/// The format is as shown.
///
/// ```rust,ignore
/// URL : { Parameter => Capture } {
///     Method => Response
/// }
/// ```
pub struct Router {
    inner: IndexMap<Method, Vec<Route>>,
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
    ///             Response::new().with_body("Hello World!").build()
    ///         });
    ///     });
    /// ```
    ///
    /// ```rust,ignore
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
    ///             Response::new().with_body(c.get("name")).build()
    ///         });
    ///     });
    /// ```
    ///
    /// ```rust,ignore
    /// "/txuritan" : { "name" => "txuritan" } {
    ///     GET => "txuritan"
    /// }
    /// ```
    pub fn route<
        S: Into<String>,
        H: Fn(Request, Arc<State>, Capture)
                -> Box<Future<Item = response::Response<Body>, Error = DireError> + Send + 'static>
            + Send
            + Sync
            + 'static,
    >(
        &mut self,
        method: Method,
        path: S,
        handler: H,
    ) {
        let path = path.into();

        // Transform the path in to ids and regex
        let reader = self.read(&path);

        self.inner.entry(method).or_insert(Vec::new()).push(Route {
            handler: Box::new(handler),
            ids: reader.0,
            path,
            pattern: reader.1,
        });
    }

    /// Adds a GET request handler.
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
    ///             Response::new().with_body("Hello World!").build()
    ///         });
    ///     });
    /// ```
    ///
    /// ```rust,ignore
    /// "/" : {  } {
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
    ///             Response::new().with_body(c.get("name")).build()
    ///         });
    ///     });
    /// ```
    ///
    /// ```rust,ignore
    /// "/txuritan" : { "name" => "txuritan" } {
    ///     GET => "txuritan"
    /// }
    /// ```
    pub fn get<
        S: Into<String>,
        H: Fn(Request, Arc<State>, Capture)
                -> Box<Future<Item = response::Response<Body>, Error = DireError> + Send + 'static>
            + Send
            + Sync
            + 'static,
    >(
        &mut self,
        path: S,
        handler: H,
    ) {
        self.route(Method::GET, path, handler);
    }

    /// Adds a POST request handler.
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
    ///             Response::new().with_body("Hello World!").build()
    ///         });
    ///     });
    /// ```
    ///
    /// ```rust,ignore
    /// "/" : {  } {
    ///     POST => "Hello World!"
    /// }
    /// ```
    pub fn post<
        S: Into<String>,
        H: Fn(Request, Arc<State>, Capture)
                -> Box<Future<Item = response::Response<Body>, Error = DireError> + Send + 'static>
            + Send
            + Sync
            + 'static,
    >(
        &mut self,
        path: S,
        handler: H,
    ) {
        self.route(Method::POST, path, handler);
    }

    /// Adds a PUT request handler.
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
    ///             Response::new().with_body("Hello World!").build()
    ///         });
    ///     });
    /// ```
    ///
    /// ```rust,ignore
    /// "/" : {  } {
    ///     PUT => "Hello World!"
    /// }
    /// ```
    pub fn put<
        S: Into<String>,
        H: Fn(Request, Arc<State>, Capture)
                -> Box<Future<Item = response::Response<Body>, Error = DireError> + Send + 'static>
            + Send
            + Sync
            + 'static,
    >(
        &mut self,
        path: S,
        handler: H,
    ) {
        self.route(Method::PUT, path, handler);
    }

    /// Adds a DELETE request handler.
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
    ///             Response::new().with_body("Hello World!").build()
    ///         });
    ///     });
    /// ```
    ///
    /// ```rust,ignore
    /// "/" : {  } {
    ///     DELETE => "Hello World!"
    /// }
    /// ```
    pub fn delete<
        S: Into<String>,
        H: Fn(Request, Arc<State>, Capture)
                -> Box<Future<Item = response::Response<Body>, Error = DireError> + Send + 'static>
            + Send
            + Sync
            + 'static,
    >(
        &mut self,
        path: S,
        handler: H,
    ) {
        self.route(Method::DELETE, path, handler);
    }

    /// Adds a HEAD request handler.
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
    ///             Response::new().with_body("Hello World!").build()
    ///         });
    ///     });
    /// ```
    ///
    /// ```rust,ignore
    /// "/" : {  } {
    ///     HEAD => "Hello World!"
    /// }
    /// ```
    pub fn head<
        S: Into<String>,
        H: Fn(Request, Arc<State>, Capture)
                -> Box<Future<Item = response::Response<Body>, Error = DireError> + Send + 'static>
            + Send
            + Sync
            + 'static,
    >(
        &mut self,
        path: S,
        handler: H,
    ) {
        self.route(Method::HEAD, path, handler);
    }

    /// Adds a OPTIONS request handler.
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
    ///             Response::new().with_body("Hello World!").build()
    ///         });
    ///     });
    /// ```
    ///
    /// ```rust,ignore
    /// "/" : {  } {
    ///     OPTIONS => "Hello World!"
    /// }
    /// ```
    pub fn options<
        S: Into<String>,
        H: Fn(Request, Arc<State>, Capture)
                -> Box<Future<Item = response::Response<Body>, Error = DireError> + Send + 'static>
            + Send
            + Sync
            + 'static,
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
    ///                 Response::new().with_body("Hello World!").build()
    ///             });
    ///         });
    ///     });
    /// ```
    ///
    /// ```rust,ignore
    /// "/parent/child" : {  } {
    ///     GET => "Hello World!"
    /// }
    /// ```
    pub fn path<S: Into<String>, F: Fn(&mut Router) + Send + Sync + 'static>(
        &mut self,
        path: S,
        sub: F,
    ) {
        let mut builder = Router::new();

        sub(&mut builder);

        let path = path.into();

        // Loop through new methods
        for (method, routes) in builder.inner {
            // Loop through new routes
            for route in routes {
                // Concatenate paths
                let n_path = format!("{}{}", path, route.path);

                // Transform the path in to ids and regex
                let reader = self.read(&n_path);

                self.inner
                    .entry(method.clone())
                    .or_insert(Vec::new())
                    .push(Route {
                        handler: route.handler,
                        ids: reader.0,
                        path: n_path,
                        pattern: reader.1,
                    });
            }
        }
    }

    /// When a request is received this is called to find a handler.
    #[inline]
    fn recognize(&self, method: &Method, path: &str) -> Result<(&Handler, Capture), StatusCode> {
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

    /// Takes each capture and transforms it into a map of ids and captures.
    #[inline]
    fn captures(&self, route: &Route, re: &Regex, path: &str) -> Option<Capture> {
        // Get captures.
        re.captures(path).map(|caps| {
            let mut captures = Capture::new();

            // Loop through each capture
            for (i, _) in caps.iter().enumerate() {
                // We dont want the first whole capture.
                if i != 0 {
                    // Insert the capture to its id.
                    captures.set(
                        // An id exists so the unwrap is safe.
                        route.ids[i - 1].as_str(),
                        // The capture exists so the unwrap is safe.
                        caps.get(i).unwrap().as_str(),
                    );
                }
            }

            captures
        })
    }

    /// Parse each path into a vector of ids and a regex pattern
    #[inline]
    fn read(&self, path: &str) -> (Vec<String>, Regex) {
        let mut ids: Vec<String> = Vec::new();
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

        (
            ids,
            match Regex::new(&self.normalize(&pattern)) {
                Ok(r) => r,
                Err(e) => {
                    eprintln!("Regex pattern error: {}", e);
                    ::std::process::exit(1);
                }
            },
        )
    }

    /// Normalizes the regex paths.
    ///
    /// Removes the beginning `^` and ending `$` and `/`, if the exist.
    /// Then adds them even if they weren't there.
    #[inline]
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

/// A wrapper around Hyper Response.
pub struct Response {
    body: Body,
    parts: response::Parts,
}

impl Response {
    /// Constructs a new Response.
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

    /// Builder set Response's HTTP headers.
    pub fn with_headers(mut self, headers: HeaderMap<HeaderValue>) -> Self {
        self.parts.headers.extend(headers);
        self
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
    /// Wrapper around Response.set_body for the HTML context type.
    pub fn html<T: Into<String>>(&mut self, html: T) {
        let _ = self
            .headers_mut()
            .insert(header::CONTENT_TYPE, HeaderValue::from_static("text/html"));

        self.set_body(html);
    }

    /// Wrapper around Response.set_body for the CSS context type.
    pub fn css<F: Fn(&mut CssBuilder)>(&mut self, css: F) {
        let _ = self
            .headers_mut()
            .insert(header::CONTENT_TYPE, HeaderValue::from_static("text/css"));

        let mut builder = CssBuilder::new();

        css(&mut builder);

        self.set_body(builder.get_body());
    }

    /// Wrapper around Response.set_body for the CSS context type.
    pub fn with_css<F: Fn(&mut CssBuilder)>(mut self, css: F) -> Self {
        self.css(css);
        self
    }

    /// Wrapper around Response.set_body for the JS context type.
    pub fn js<F: Fn(&mut JsBuilder)>(&mut self, js: F) {
        let _ = self.headers_mut().insert(
            header::CONTENT_TYPE,
            HeaderValue::from_static("application/javascript"),
        );

        let mut builder = JsBuilder::new();

        js(&mut builder);

        self.set_body(builder.get_body());
    }

    /// Wrapper around Response.set_body for the JS context type.
    pub fn with_js<F: Fn(&mut JsBuilder)>(mut self, js: F) -> Self {
        self.js(js);
        self
    }

    /// Wrapper around Response.set_body for the JSON context type.
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

    /// Builder function for Json responses.
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
    pub fn into_hyper(self) -> response::Response<Body> {
        response::Response::from_parts(self.parts, self.body)
    }

    /// Wrapper around 'into_hyper' to change it into a future response.
    pub fn build(
        self,
    ) -> Box<Future<Item = response::Response<Body>, Error = DireError> + Send + 'static> {
        Box::new(future::ok(self.into_hyper()))
    }
}

impl Default for Response {
    fn default() -> Response {
        let (parts, body) = hyper::Response::new(Body::empty()).into_parts();
        Response { body, parts }
    }
}

/// A builder function for CSS Responses.
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

    /// Load from File.
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

    /// Load from File.
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
#[cfg(feature = "json")]
pub struct JsonBuilder<T: Serialize + Send + Sync> {
    /// Json response wrapper to be sent.
    wrapper: Wrapper<T>,
}

#[cfg(feature = "json")]
impl JsonBuilder<()> {
    /// Creates a JsonBuilder with given type.
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
    pub fn error<S: Into<String>>(&mut self, message: S) {
        self.wrapper.add_message(message);
    }

    /// Added an error message to the wrapper.
    pub fn errors<S: Into<String>>(&mut self, messages: Vec<S>) {
        for message in messages {
            self.wrapper.add_message(message);
        }
    }

    /// Set the status code of the Json response.
    ///
    /// This can be gotten with StatusCode.as_u16.
    pub fn code(&mut self, status: u16) {
        self.wrapper.set_code(status);
    }

    /// Set the status code of the Json response.
    ///
    /// This can be gotten with StatusCode.as_u16.
    pub fn with_code(mut self, status: u16) -> Self {
        self.code(status);
        self
    }

    /// Set the status string of the Json response.
    ///
    /// This can be gotten with StatusCode.as_str.
    pub fn status<S: Into<String>>(&mut self, status: S) {
        self.wrapper.set_status(status);
    }

    /// Set the status string of the Json response.
    ///
    /// This can be gotten with StatusCode.as_str.
    pub fn with_status(mut self, status: &str) -> Self {
        self.status(status);
        self
    }

    fn get_body(&self) -> String {
        serde_json::to_string(&self.wrapper).expect("Can not transform struct into json")
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
    /// Constructs a new Wrapper.
    fn new() -> Wrapper<T> {
        Wrapper::default()
    }

    /// Add error message to response wrapper.
    fn add_message<S: Into<String>>(&mut self, message: S) {
        self.messages.push(message.into());
    }

    /// Set the response status code.
    fn set_code(&mut self, code: u16) {
        self.code = code;
    }

    /// Set the response status string.
    fn set_status<S: Into<String>>(&mut self, status: S) {
        self.status = status.into();
    }

    /// Set the wrapped response.
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

/// A wrapper around Hyper Request.
pub struct Request {
    body: Body,
    parts: request::Parts,
}

impl Request {
    /// Constructs a new Request.
    fn new(body: Body, parts: request::Parts) -> Self {
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

    /// Return Request body.
    pub fn into_body(self) -> Body {
        self.body
    }
}

/// Creates a HeaderMap from a list of key-value pairs.
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
///             Response::new()
///                 .with_headers(headermap! {
///                     header::CONTENT_TYPE => "text/plain",
///                 }).with_body("Hello World!")
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
                let _ = _map.insert($key, ::direkuta::prelude::hyper::HeaderValue::from_str($value).unwrap());
            )*
            _map
        }
    };
}

/// Imports just the required parts of Direkuta.
pub mod prelude {
    pub use super::{Capture, Direkuta, DireError, Logger, Middle, Request, Response, State};

    /// Imports all builders used in Direkuta.
    ///
    /// Useful for turing the closures into stand-alone functions.
    pub mod builder {
        pub use super::super::{Config, CssBuilder, JsBuilder, JsonBuilder, Router};
    }

    /// Imports the required parts from Tera.
    ///
    /// You'll need to import this if you want to use Tera templates.
    #[cfg(feature = "html")]
    pub mod html {
        pub use tera::{Context, Tera};
    }

    /// Imports the required parts from Hyper.
    ///
    /// You'll need this if you want to create a handler that doesn't have a function
    /// or if you want to set response Headers.
    pub mod hyper {
        pub use hyper::header::{self, HeaderMap, HeaderValue};
        pub use hyper::Method;
    }

    /// Exports Futures' 'future', 'Future', and 'Stream'.
    #[cfg(feature = "runtime")]
    pub mod rt {
        use http::response::Response;
        use hyper::Body;

        use super::super::DireError;

        /// Wrapper around HTTP Response
        pub type Res = Response<Body>;

        /// Type alias for Router returns.
        pub type FutureResponse = Box<Future<Item = Res, Error = DireError> + Send + 'static>;
        pub use futures::{future, Future, Stream};
    }
}
