use libfsm::pcre;

pcre!(find_john, "john");
pcre!(find_jim, "jim");
pcre!(find_jimmy, b"jimmy");
pcre!(find_john_or_jim, "john|jim");
pcre!(find_space, " ");

fn main() {
    if find_john("does this string contain john?".bytes()).is_some() {
        println!("found john");
    } else {
        println!("did not find john");
    }

    if find_jim("this string does not contain the forbidden name".bytes()).is_some() {
        println!("found jim");
    } else {
        println!("did not find jim");
    }

    if find_john_or_jim("this string has jim in it".bytes()).is_some() {
        println!("found one of them");
    } else {
        println!("did not find one of them");
    }

    if find_space("this string has a space".bytes()).is_some() {
        println!("found a space");
    } else {
        println!("did not find a space");
    }
}
