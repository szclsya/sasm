#[macro_export]
macro_rules! msg {
    ($prefix:tt, $($arg:tt)+) => {
        print!("{:>9} ", $prefix);
        println!($($arg)+);
    };
}

#[macro_export]
macro_rules! success {
    ($($arg:tt)+) => {
        print!("{:>9} ", console::style("SUCCESS").green().bold());
        println!($($arg)+);
    };
}

#[macro_export]
macro_rules! info {
    ($($arg:tt)+) => {
        print!("{:>9} ", console::style("INFO").blue().bold());
        println!($($arg)+);
    };
}

#[macro_export]
macro_rules! warn {
    ($($arg:tt)+) => {
        print!("{:>9} ", console::style("WARNING").yellow().bold());
        println!($($arg)+);
    };
}

#[macro_export]
macro_rules! error {
    ($($arg:tt)+) => {
        print!("{:>9} ", console::style("ERROR").red().bold());
        println!($($arg)+);
    };
}

#[macro_export]
macro_rules! due_to {
    ($($arg:tt)+) => {
        print!("{:>9} ", console::style("DUE TO").yellow().bold());
        println!($($arg)+);
    };
}
