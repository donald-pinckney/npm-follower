

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

    if pids.len() != 0 {
        println!("Cannot run download_queuer concurrently. Already running with PIDs: {}", 
            pids.iter()
                .map(|pid| format!("{}", pid))
                .collect::<Vec<_>>()
                .join(" "));
        std::process::exit(1);
    }
}