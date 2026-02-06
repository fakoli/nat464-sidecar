use tokio::io::{self, AsyncRead, AsyncWrite};
use tracing::debug;

/// Bidirectional byte copying between two streams.
///
/// Uses tokio::io::copy_bidirectional which internally uses an efficient
/// buffer-based approach. On Linux, future versions could use splice(2)
/// for true zero-copy (Milestone 2).
///
/// Returns the total bytes transferred in each direction: (client→server, server→client).
pub async fn bidirectional_copy<A, B>(mut a: A, mut b: B) -> io::Result<(u64, u64)>
where
    A: AsyncRead + AsyncWrite + Unpin,
    B: AsyncRead + AsyncWrite + Unpin,
{
    let result = io::copy_bidirectional(&mut a, &mut b).await;
    match &result {
        Ok((a_to_b, b_to_a)) => {
            debug!(a_to_b, b_to_a, "connection closed, bytes transferred");
        }
        Err(e) => {
            debug!(error = %e, "connection error during copy");
        }
    }
    result
}
