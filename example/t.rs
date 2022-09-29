mod a;
#[path = "b.rs"]
mod b;

fn main() {
    println!("{}", a::c::f() + b::c::f());
}
