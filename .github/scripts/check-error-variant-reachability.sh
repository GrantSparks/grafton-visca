#!/usr/bin/env bash
set -euo pipefail

repository_root="${ERROR_REACHABILITY_REPOSITORY_ROOT:-$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)}"
cd "${repository_root}"

mapfile -t declared_variants < <(
  sed -n '/^pub enum Error {$/,/^}$/p' src/error.rs |
    sed -nE 's/^    ([A-Z][A-Za-z0-9_]*)([ ({,].*)?$/\1/p'
)

if ((${#declared_variants[@]} == 0)); then
  echo "No public Error variants were found in src/error.rs" >&2
  exit 1
fi

# A textual `Error::Variant` search cannot prove reachability: matches, docs,
# and classification tables all mention variants without constructing them.
# Keep one reviewed production constructor for every public variant instead.
# Moving or replacing a constructor intentionally requires updating this
# inventory; deleting it makes the release gate fail even when a pattern match
# with the same spelling remains elsewhere (#736).
declare -A reachable=()

record_construction() {
  local variant="$1"
  local path="$2"
  local expression="$3"

  if [[ -n "${reachable[${variant}]+present}" ]]; then
    echo "Duplicate Error::${variant} construction inventory entry" >&2
    exit 1
  fi
  if [[ ! -f "${path}" ]] || ! grep -Fq -- "${expression}" "${path}"; then
    echo "Missing Error::${variant} production constructor:" >&2
    echo "  ${path}: ${expression}" >&2
    exit 1
  fi
  reachable["${variant}"]="${path}"
}

record_construction ConnectionFailed src/transport/tokio/serial.rs \
  '.map_err(|e| Error::ConnectionFailed {'
record_construction ConnectionClosed src/runtime/owner/mod.rs \
  'ShutdownReason::TransportClosed { reason } => Error::ConnectionClosed {'
record_construction CommandPending src/command/response/types.rs \
  'Response::CmdAck { .. } => Err(Error::CommandPending)'
record_construction InvalidResponse src/command/response/types.rs \
  'Response::Unknown { data, .. } => Err(Error::InvalidResponse {'
record_construction FeatureNotSupported src/prepared.rs \
  'return Err(Error::FeatureNotSupported {'
record_construction Io src/error.rs \
  'Self::Io(Arc::new(err))'
record_construction SyntaxError src/error.rs \
  '0x02 => Self::SyntaxError,'
record_construction CommandBufferFull src/error.rs \
  '0x03 => Self::CommandBufferFull,'
record_construction CommandCanceled src/error.rs \
  '0x04 => Self::CommandCanceled,'
record_construction NoSocket src/error.rs \
  '0x05 => Self::NoSocket,'
record_construction CommandNotExecutable src/error.rs \
  '0x41 => Self::CommandNotExecutable,'
record_construction InvalidResponseFormat src/command/response/payload.rs \
  'return Err(Error::InvalidResponseFormat);'
record_construction InvalidResponseLength src/error.rs \
  'Self::InvalidResponseLength {'
record_construction UnexpectedResponseType src/command/inquiry_structs.rs \
  '_ => Err(Error::UnexpectedResponseType),'
record_construction Unknown src/error.rs \
  '_ => Self::Unknown(code),'
record_construction InvalidRequest src/raw.rs \
  'Error::InvalidRequest(Cow::Borrowed('
record_construction MessageLengthError src/error.rs \
  '0x01 => Self::MessageLengthError,'
record_construction ParseError src/error.rs \
  'Self::ParseError(Cow::Owned(err.to_string()))'
record_construction TransportError src/runtime/owner/mod.rs \
  'Error::TransportError(format!("datagram send failed: {error}").into())'
record_construction InvalidParameter src/types.rs \
  'Err(Error::InvalidParameter {'
record_construction BufferTooSmall src/raw.rs \
  'return Err(Error::BufferTooSmall {'
record_construction InvalidPreset src/camera/profiles.rs \
  'Err(Error::InvalidPreset {'
record_construction ParameterOutOfRange src/units.rs \
  'return Err(Error::ParameterOutOfRange {'
record_construction Timeout src/runtime/engine/mod.rs \
  'RuntimeOutcome::Failed(last_error.unwrap_or(Error::Timeout))'
record_construction MaxRetriesExceeded src/transport/serial/handshake.rs \
  'Err(Error::MaxRetriesExceeded)'
record_construction NotSupported src/runtime/owner/adapter.rs \
  'return Err(Error::NotSupported);'
record_construction InvalidState src/async_session.rs \
  'Error::InvalidState("session has no registered target".into())'
record_construction TransportBusy src/runtime/owner/blocking.rs \
  '.map_err(|_| Error::TransportBusy)?;'
record_construction RuntimeShutdown src/runtime/engine/mod.rs \
  'self.terminate_session(SessionState::Shutdown, Error::RuntimeShutdown, effects)'
record_construction CancellationUnconfirmed src/runtime/engine/mod.rs \
  'Error::CancellationUnconfirmed'
record_construction UnsequencedCommandUnconfirmed src/runtime/engine/mod.rs \
  'RuntimeOutcome::Failed(Error::UnsequencedCommandUnconfirmed)'
record_construction RuntimeIdentityExhausted src/runtime/engine/mod.rs \
  'error: Error::RuntimeIdentityExhausted,'
record_construction RuntimeQueueFull src/runtime/engine/mod.rs \
  'error: Error::RuntimeQueueFull {'
record_construction StreamPoisoned src/runtime/engine/mod.rs \
  'Error::StreamPoisoned {'
record_construction DecoderNotFound src/command/inquiry_structs.rs \
  'return Err(Error::DecoderNotFound {'
record_construction InvalidCameraId src/camera_id.rs \
  '_ => Err(Error::InvalidCameraId { id }),'
record_construction ResponseTooLarge src/raw.rs \
  'return Err(Error::ResponseTooLarge {'
record_construction InquiryNotCancelable src/runtime/engine/mod.rs \
  'observation: CancellationObservation::Failed(Error::InquiryNotCancelable),'
record_construction InvalidAddress src/transport/address.rs \
  'Error::InvalidAddress {'
record_construction UnsupportedTransport src/runtime/owner/adapter.rs \
  'return Err(Error::UnsupportedTransport {'
record_construction MissingRuntime src/executor.rs \
  '.map_err(|_| Error::MissingRuntime)'
record_construction WithContext src/error.rs \
  'Self::WithContext {'

missing=()
for variant in "${declared_variants[@]}"; do
  if [[ -z "${reachable[${variant}]+present}" ]]; then
    missing+=("${variant}")
  fi
done

if ((${#missing[@]} != 0)); then
  echo "Public Error variants without an inventoried production constructor:" >&2
  printf '  Error::%s\n' "${missing[@]}" >&2
  echo "Delete each dead variant or inventory its real production construction path before the RC API locks." >&2
  exit 1
fi

if ((${#reachable[@]} != ${#declared_variants[@]})); then
  echo "Error construction inventory contains an entry not declared by public Error" >&2
  exit 1
fi

printf 'Verified explicit production constructors for %d public Error variants.\n' \
  "${#declared_variants[@]}"
