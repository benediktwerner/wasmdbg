use crate::Debugger;

pub struct Command {
    pub name: &'static str,
    pub abrvs: Vec<&'static str>,
    pub help: &'static str,
    pub requires_file: bool,
    pub requires_running: bool,
    pub argc: usize,
    pub handler: &'static Fn(&mut Debugger, &Vec<&str>),
}

impl Command {
    pub fn new(name: &'static str, handler: &'static Fn(&mut Debugger, &Vec<&str>)) -> Command {
        Command {
            name,
            handler,
            abrvs: Vec::new(),
            help: "No help for this command",
            argc: 0,
            requires_file: false,
            requires_running: false,
        }
    }

    pub fn handle(&self, dbg: &mut Debugger, args: &Vec<&str>) {
        if args.len() != self.argc {
            println!(
                "Invalid number of arguments! \"{}\" takes exactly {} args.",
                self.name, self.argc
            );
            return;
        }
        (self.handler)(dbg, args);
    }

    pub fn abrv(mut self, abrv: &'static str) -> Self {
        self.abrvs.push(abrv);
        self
    }

    pub fn help(mut self, help: &'static str) -> Self {
        self.help = help;
        self
    }

    pub fn takes_args(mut self, argc: usize) -> Self {
        self.argc = argc;
        self
    }

    pub fn requires_file(mut self) -> Self {
        self.requires_file = true;
        self
    }

    pub fn requires_running(mut self) -> Self {
        self.requires_running = true;
        self
    }

    pub fn name(&self) -> &str {
        self.name
    }
}
