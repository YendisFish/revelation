/*
 * This will eventually help processes
 * open up IO for subprocesses.
 */

pub struct IoHandle {

}

impl IoHandle {
    pub fn new() {

    }
}

pub struct Logger {
    pub name: String,
    pub time: bool, // eventually indicates if logger should output time
    pub extra: Vec<Box<dyn Fn() -> String>>
}

impl Logger {
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            time: false,
            extra: vec![],
        }
    }

    pub fn write(&self, text: impl Into<String>) {
        let mut header = String::new();
        header += format!("[{}] ", self.name).as_str();

        for fun in &self.extra {
            let out = fun();
            header += format!("[{}] ", out).as_str();
        }

        header += text.into().as_str();
        println!("{}", header);
    }
}
