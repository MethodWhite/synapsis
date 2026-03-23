//! HTTP Server

pub struct HTTPServer {
    port: u16,
}

impl HTTPServer {
    pub fn new(port: u16) -> Self {
        Self { port }
    }
    pub fn start(&self) {
        println!("HTTP Server starting on port {}", self.port);
    }
}

impl std::fmt::Debug for HTTPServer {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("HTTPServer")
            .field("port", &self.port)
            .finish()
    }
}
