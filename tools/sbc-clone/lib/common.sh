#!/usr/bin/env bash
# Shared helpers for sbc-clone. Sourced on the source server (capture) and on
# the target server (apply/validate), so it must not depend on either side.

C_RED=$'\e[31m'; C_GRN=$'\e[32m'; C_YLW=$'\e[33m'; C_BLU=$'\e[34m'; C_BLD=$'\e[1m'; C_RST=$'\e[0m'
[[ -t 1 ]] || { C_RED=; C_GRN=; C_YLW=; C_BLU=; C_BLD=; C_RST=; }

LOG_FILE=${LOG_FILE:-/dev/null}

_log() { printf '%s %s\n' "$(date '+%F %T')" "$*" >> "$LOG_FILE"; }
log_section() { printf '\n%s== %s ==%s\n' "$C_BLD$C_BLU" "$*" "$C_RST"; _log "== $*"; }
log_info()    { printf '   %s\n' "$*"; _log "INFO $*"; }
log_ok()      { printf '   %s✔%s %s\n' "$C_GRN" "$C_RST" "$*"; _log "OK   $*"; }
log_warn()    { printf '   %s⚠%s %s\n' "$C_YLW" "$C_RST" "$*" >&2; _log "WARN $*"; }
log_err()     { printf '   %s✘%s %s\n' "$C_RED" "$C_RST" "$*" >&2; _log "ERR  $*"; }
die()         { log_err "$*"; exit 1; }

# run a command, logging its output; never aborts the caller
run_logged() {
    _log "RUN  $*"
    "$@" >> "$LOG_FILE" 2>&1
}

gen_secret() {
    # url/env/sql safe: letters and digits only
    # no `tr </dev/urandom | head`: head's early exit is a SIGPIPE failure
    # under `set -o pipefail`
    local len=${1:-32} s=""
    while (( ${#s} < len )); do s+=$(openssl rand -base64 $((len * 2)) | tr -dc 'A-Za-z0-9'); done
    printf '%s' "${s:0:len}"
}

is_ipv4() {
    local ip=$1 o
    [[ $ip =~ ^([0-9]{1,3})\.([0-9]{1,3})\.([0-9]{1,3})\.([0-9]{1,3})$ ]] || return 1
    for o in "${BASH_REMATCH[@]:1}"; do (( o <= 255 )) || return 1; done
}

# IP_MAP holds "old=new" pairs. rewrite_ips replaces every old IP as a whole
# token (10.118.0.6 never matches 10.118.0.60 or 110.118.0.6) in one pass, so
# a new IP that equals some other old IP is never rewritten twice.
declare -ga IP_MAP=()

rewrite_ips() {
    (( ${#IP_MAP[@]} )) || return 0
    local f files=()
    for f in "$@"; do
        [[ -f "$f" && ! -L "$f" ]] && grep -Iq . "$f" 2>/dev/null && files+=("$f")
    done
    (( ${#files[@]} )) || return 0
    IP_MAP_STR="${IP_MAP[*]}" perl -i -pe '
        BEGIN {
            for (split / /, $ENV{IP_MAP_STR}) { my ($o,$n) = split /=/; $m{$o} = $n if $o ne $n; }
            $re = join "|", map { quotemeta } sort { length($b) <=> length($a) } keys %m;
        }
        s/(?<![\d.])($re)(?!\d)(?!\.\d)/$m{$1}/g if $re;
    ' "${files[@]}"
}

# role-based old→new IP pairs from the SRC_*/NEW_* variables
build_ip_map() {
    # generic replacement: where the source used one IP for two roles
    # (e.g. signalling and RTP on the same LAN IP), the signalling role wins
    # and the agents set the RTP-specific places explicitly.
    IP_MAP=()
    [[ -n ${SRC_RTP_INT:-} ]] && IP_MAP+=("$SRC_RTP_INT=$NEW_RTP_INT")
    [[ -n ${SRC_RTP_EXT:-} ]] && IP_MAP+=("$SRC_RTP_EXT=$NEW_RTP_EXT")
    IP_MAP+=("$SRC_SIG_EXT=$NEW_SIG_EXT" "$SRC_SIG_INT=$NEW_SIG_INT")
    [[ -n ${SRC_MGMT:-} ]] && IP_MAP+=("$SRC_MGMT=$NEW_MGMT")
    # later pairs override earlier ones for the same old IP
    local -A m=(); local p
    for p in "${IP_MAP[@]}"; do m[${p%%=*}]=${p#*=}; done
    IP_MAP=(); for p in "${!m[@]}"; do IP_MAP+=("$p=${m[$p]}"); done
}

# rewrite_ips over every text file below a directory
rewrite_ips_tree() {
    local dir=$1
    [[ -d "$dir" ]] || return 0
    local -a files
    mapfile -d '' files < <(find "$dir" -type f -size -20M -print0)
    (( ${#files[@]} )) && rewrite_ips "${files[@]}"
}

# set KEY=value in a dotenv file (adds it when missing)
set_env_var() {
    local file=$1 key=$2 val=$3
    [[ -f "$file" ]] || return 0
    if grep -q "^${key}=" "$file"; then
        KEY="$key" VAL="$val" perl -i -pe 's/^\Q$ENV{KEY}\E=.*$/$ENV{KEY}=$ENV{VAL}/' "$file"
    else
        printf '%s=%s\n' "$key" "$val" >> "$file"
    fi
}

# replace the password of user USER in every scheme://USER:PASS@ URL of a file
set_url_password() {
    local file=$1 user=$2 pass=$3
    [[ -f "$file" ]] || return 0
    U="$user" P="$pass" perl -i -pe 's{(://\Q$ENV{U}\E:)[^@\s"'"'"']*@}{$1$ENV{P}\@}g' "$file"
}

# validation bookkeeping (target side)
CHECKS_TOTAL=0; CHECKS_OK=0; CHECKS_WARN=0
declare -ga CHECK_LINES=()
check() {
    # check "description" command...  -> pass/fail, never aborts
    local desc=$1; shift
    CHECKS_TOTAL=$((CHECKS_TOTAL + 1))
    if "$@" >> "$LOG_FILE" 2>&1; then
        CHECKS_OK=$((CHECKS_OK + 1)); CHECK_LINES+=("[OK]    $desc"); log_ok "$desc"
    else
        CHECK_LINES+=("[FALLA] $desc"); log_err "$desc"
    fi
}
check_warn() {
    # non-blocking check: counts as passed but is reported as a warning
    local desc=$1; shift
    CHECKS_TOTAL=$((CHECKS_TOTAL + 1))
    if "$@" >> "$LOG_FILE" 2>&1; then
        CHECKS_OK=$((CHECKS_OK + 1)); CHECK_LINES+=("[OK]    $desc"); log_ok "$desc"
    else
        CHECKS_OK=$((CHECKS_OK + 1)); CHECKS_WARN=$((CHECKS_WARN + 1))
        CHECK_LINES+=("[AVISO] $desc"); log_warn "$desc"
    fi
}

# copy the CONTENTS of src into dst without changing the mode/owner of
# directories that already exist in dst (cp -a src/. dst/ would)
install_tree() {
    local src=$1 dst=$2
    [[ -d $src ]] || return 0
    mkdir -p "$dst"
    (cd "$src" && tar -cf - .) | tar -C "$dst" -xpf - --no-overwrite-dir
}

# SECRET_MAP holds "base64(old):base64(new)" pairs. replace_old_secrets swaps
# every occurrence of an old password for its new one in the given files.
replace_old_secrets() {
    [[ -n ${SECRET_MAP:-} ]] || return 0
    local f files=()
    for f in "$@"; do
        [[ -f "$f" && ! -L "$f" ]] && grep -Iq . "$f" 2>/dev/null && files+=("$f")
    done
    (( ${#files[@]} )) || return 0
    SECRET_MAP="$SECRET_MAP" perl -MMIME::Base64 -i -pe '
        BEGIN {
            for (split / /, $ENV{SECRET_MAP}) { my ($o,$n) = split /:/; $m{decode_base64($o)} = decode_base64($n); }
            $re = join "|", map { quotemeta } sort { length($b) <=> length($a) } keys %m;
        }
        s/($re)/$m{$1}/g if $re;
    ' "${files[@]}"
}
