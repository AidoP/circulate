use circulate::RingBuffer;

fn main() {
    let mut fruit = RingBuffer::new();
    fruit.push("apples".to_owned());
    fruit.push("oranges".to_owned());
    fruit.push("pears".to_owned());
    fruit.push("grapes".to_owned());
    for f in fruit.iter_mut() {
        f.make_ascii_uppercase()
    }
    
    println!("I LOVE {}!", fruit.pop().as_deref().unwrap_or("nothing ☹️"));

    for f in fruit.iter() {
        println!("I HATE {f}!")
    }
}