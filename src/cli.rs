#[macro_export]
macro_rules! println_info {
    ($($arg:tt)+) => {
        print!("{} ", console::style("INFO ").on_blue().bright());
        println!($($arg)+);
    };
}

#[macro_export]
macro_rules! println_warn {
    ($($arg:tt)+) => {
        print!("{} ", console::style("WARN ").on_orange().bright());
        println!($($arg)+);
    };
}

#[macro_export]
macro_rules! println_error {
    ($($arg:tt)+) => {
        print!("{} ", console::style("ERROR").on_red().bright());
        println!($($arg)+);
    };
}
