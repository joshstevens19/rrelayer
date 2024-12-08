pub type AccessToken = String;
pub type RefreshToken = String;

pub enum JwtTokenType {
    Access,
    Refresh,
}
