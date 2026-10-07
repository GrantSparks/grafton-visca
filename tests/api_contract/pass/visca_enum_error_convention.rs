use grafton_visca::ViscaEnum;

mod named_error {
    #[derive(Debug, PartialEq)]
    pub struct Error(pub String, pub Vec<u8>);
    impl Error {
        pub fn invalid_response(expected: impl Into<std::borrow::Cow<'static, str>>, actual: Vec<u8>) -> Self {
            Self(expected.into().into_owned(), actual)
        }
    }
}
#[derive(Debug, PartialEq, ViscaEnum)]
#[visca_enum(error_type = named_error::Error)]
enum Named { Valid = 1 }

#[derive(Debug, PartialEq)]
struct MyError(String);
impl From<String> for MyError {
    fn from(message: String) -> Self { Self(message) }
}
#[derive(Debug, PartialEq, ViscaEnum)]
#[visca_enum(error_type = MyError)]
enum Custom { Valid = 1 }

#[derive(Debug, PartialEq, ViscaEnum)]
#[visca_enum(error_type = grafton_visca::Error)]
enum Explicit { Valid = 1 }

fn main() {
    assert_eq!(Named::try_from(1).unwrap(), Named::Valid);
    assert_eq!(Named::try_from(2).unwrap_err().1, vec![2]);
    assert!(Custom::try_from(2).unwrap_err().0.contains("Invalid value"));
    assert!(matches!(Explicit::try_from(2), Err(grafton_visca::Error::InvalidResponse { actual, .. }) if actual == vec![2]));
}
