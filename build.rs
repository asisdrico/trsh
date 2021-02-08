use std::process::Command;
use std::process::Stdio;

fn main() {
    //let trsh_id = get_random_string("32");
    //let trsh_key = get_random_string("32");
    //let trsh_iv = get_random_string("8");
    let trsh_id = "ohpie2naiwoo1lah6aeteexi5beiRas7";
    let trsh_key = "Fahm9Oruet8zahcoFahm9Oruet8zahco";
    let trsh_iv = "biTh0eoY";
    println!("cargo:rustc-env=TRSH_ID={}", trsh_id);
    println!("cargo:rustc-env=TRSH_KEY={}", trsh_key);
    println!("cargo:rustc-env=TRSH_IV={}", trsh_iv);
}

fn get_random_string(length: &str) -> String {
    let mut cat_child = Command::new("head")
        .args(&["-c", "1024", "/dev/urandom"])
        .stdout(Stdio::piped())
        .spawn()
        .unwrap();

    cat_child.wait().unwrap();

    if let Some(cat_output) = cat_child.stdout.take() {
        let mut tr_child = Command::new("tr")
            .arg("-dc")
            .arg("[:alnum:]")
            .stdin(cat_output)
            .stdout(Stdio::piped())
            .spawn()
            .unwrap();

        if let Some(tr_output) = tr_child.stdout.take() {
            let head_output_child = Command::new("head")
                .args(&["-c", length])
                .stdin(tr_output)
                .stdout(Stdio::piped())
                .spawn()
                .unwrap();

            let head_stdout = head_output_child.wait_with_output().unwrap();

            return String::from_utf8(head_stdout.stdout).unwrap();
        }
    }

    return "".to_string();
}
