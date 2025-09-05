#[test]
fn test_pollster_with_ready() {
    println!("Testing pollster with Ready future");
    let ready_fut = std::future::ready(42);
    println!("About to call pollster::block_on");
    let result = pollster::block_on(ready_fut);
    println!("Result: {}", result);
    assert_eq!(result, 42);
}
