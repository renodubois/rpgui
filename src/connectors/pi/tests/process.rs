use super::*;
fn fake(source: &str) -> Client {
    let mut command = Command::new("node");
    command
        .arg("-e")
        .arg(source)
        .env_clear()
        .env("PATH", std::env::var_os("PATH").unwrap());
    Client::spawn(command).unwrap()
}
fn until(client: &Client, predicate: impl Fn(&[Update]) -> bool) -> Vec<Update> {
    let deadline = Instant::now() + Duration::from_secs(5);
    let mut output = Vec::new();
    loop {
        output.extend(client.drain());
        if predicate(&output) {
            return output;
        }
        assert!(Instant::now() < deadline, "controlled transport deadline");
        thread::sleep(Duration::from_millis(5));
    }
}
#[test]
fn concurrent_real_pipes_correlate_reversed_receipts_and_drain_large_diagnostics() {
    const SCRIPT: &str = "let data='';const requests=[];process.stderr.write('x'.repeat(4*1024*1024));process.stdin.on('data',chunk=>{data+=chunk;let i;while((i=data.indexOf('\\n'))>=0){requests.push(JSON.parse(data.slice(0,i)));data=data.slice(i+1);if(requests.length===2){for(const r of requests.reverse())process.stdout.write(JSON.stringify({type:'response',id:r.id,command:r.type,success:true,data:{thread:'isolated'}})+'\\n');}}});";
    let mut a = fake(SCRIPT);
    let mut b = fake(SCRIPT);
    for client in [&mut a, &mut b] {
        client.send(json!({"type":"get_state"})).unwrap();
        client.send(json!({"type":"get_messages"})).unwrap();
    }
    for client in [&a, &b] {
        let output = until(client, |items| items.len() >= 2);
        assert!(output.iter().all(|item| matches!(item, Update::Record(_))));
    }
    a.close();
    b.close();
    for client in [&a, &b] {
        assert!(
            until(client, |items| items
                .iter()
                .any(|u| matches!(u, Update::Closed | Update::Failed(_))))
            .iter()
            .any(|u| matches!(u, Update::Closed))
        );
    }
}
#[test]
fn crash_after_dispatch_is_uncertain_without_retry() {
    let mut client = fake("process.stdin.once('data',()=>process.exit(1));");
    client
        .send(json!({"type":"prompt","message":"mutation"}))
        .unwrap();
    assert!(
        until(&client, |items| items
            .iter()
            .any(|u| matches!(u, Update::Failed(_))))
        .iter()
        .any(|u| matches!(u, Update::Failed(_)))
    );
    assert!(
        client
            .send(json!({"type":"prompt","message":"never replay"}))
            .is_err()
    );
}
#[test]
fn uncorrelated_response_closes_admission_and_terminal_failure_stays_sticky_after_drain() {
    let mut client = fake(
        "process.stdin.on('data',()=>process.stdout.write(JSON.stringify({type:'response',id:'stale',command:'prompt',success:true})+'\\n'));process.stdin.on('end',()=>process.exit(0));",
    );
    client
        .send(json!({"type":"prompt","message":"once"}))
        .unwrap();
    until(&client, |items| {
        items.iter().any(|u| matches!(u, Update::Failed(_)))
    });
    assert!(client.send(json!({"type":"get_state"})).is_err());
    thread::sleep(Duration::from_millis(30));
    assert!(client.drain().iter().all(|u| !matches!(u, Update::Closed)));
}
