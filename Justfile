set positional-arguments

login user:
  sudo ./target/debug/polyjuice --username {{user}}

buildlogin user:
  cargo build
  sudo ./target/debug/polyjuice --username {{user}}