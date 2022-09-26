use serde_json::{Map, Value};

pub fn check_no_concurrent_processes(name: &str) {
    use std::collections::HashSet;
    use std::process::Command;

    // Get the PID of this process
    let my_pid = std::process::id();

    // Run pidof to get the PIDs of all processes with the given name.
    let pidof_output = Command::new("pidof")
        .arg(name)
        .output()
        .expect("failed to execute process");
    
    // See if there are any matching PIDs other than this processe's PID.
    let mut pids: HashSet<_> = String::from_utf8(pidof_output.stdout).unwrap().split_whitespace().map(|s| s.parse::<u32>().unwrap()).collect();
    pids.remove(&my_pid);

    if !pids.is_empty() {
        println!("Cannot run download_queuer concurrently. Already running with PIDs: {}", 
            pids.iter()
                .map(|pid| format!("{}", pid))
                .collect::<Vec<_>>()
                .join(" "));
        std::process::exit(1);
    }
}



pub trait RemoveInto {
    fn remove_key<T>(&mut self, key: &'static str) -> Option<Result<T, serde_json::Error>> where T: for<'de> serde::de::Deserialize<'de>;

    fn remove_key_unwrap_type<T>(&mut self, key: &'static str) -> Option<T> where T: for<'de> serde::de::Deserialize<'de> {
        self.remove_key(key).map(|x| x.unwrap())
    }
}

impl RemoveInto for Map<String, Value> {
    fn remove_key<T>(&mut self, key: &'static str) -> Option<Result<T, serde_json::Error>> where T: for<'de> serde::de::Deserialize<'de> { 
        self.remove(key).map(|x| serde_json::from_value(x))
    }
}


pub trait FilterJsonCases: Sized {
    fn null_to_none(self) -> Option<Self>;
    fn empty_array_to_none(self) -> Option<Self>;
}

impl FilterJsonCases for Value {
    fn null_to_none(self) -> Option<Self> {
        match self {
            Value::Null => None,
            _ => Some(self)
        }
    }

    fn empty_array_to_none(self) -> Option<Self> {
        match self {
            Value::Array(xs) if xs.is_empty() => None,
            _ => Some(self)
        }
    }
}