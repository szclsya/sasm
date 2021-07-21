#[macro_export]
macro_rules! success {
    ($($arg:tt)+) => {
        print!("{} ", console::style(" SUCCESS").green().bold());
        println!($($arg)+);
    };
}

#[macro_export]
macro_rules! info {
    ($($arg:tt)+) => {
        print!("{} ", console::style("    INFO").blue().bold());
        println!($($arg)+);
    };
}

#[macro_export]
macro_rules! warn {
    ($($arg:tt)+) => {
        print!("{} ", console::style(" WARNING").yellow().bold());
        println!($($arg)+);
    };
}

#[macro_export]
macro_rules! error {
    ($($arg:tt)+) => {
        print!("{} ", console::style("   ERROR").red().bold());
        println!($($arg)+);
    };
}

#[macro_export]
macro_rules! due_to {
    ($($arg:tt)+) => {
        print!("{} ", console::style("  DUE TO").yellow().bold());
        println!($($arg)+);
    };
}
