#!/usr/bin/env bash
#===============================================================================
# ApoloBilling — UI theme applier
#
# Reads images and optional overrides from /opt/ApoloBilling/UX-UI/, derives a
# color palette, and rewrites the frontend so the new look takes effect after
# a rebuild.
#
# Conventions in the source directory (filename matching is case-insensitive
# and prefix-based, so `logo.png`, `logo-white.png`, `LOGO_v2.svg` all match):
#
#   logo*.{png,svg,jpg,jpeg,webp}      → copied verbatim to
#                                        frontend/public/logo.png
#   paleta*.{png,jpg,jpeg,webp} or
#   palette*.{png,jpg,jpeg,webp}       → image whose dominant colors drive the
#                                        generated palette. If missing, the
#                                        logo itself is used as the source.
#   favicon*.{png,ico,svg}             → copied to frontend/public/favicon.<ext>
#   palette.txt  (optional)            → manual hex overrides, one per line.
#                                        Order (any omitted gets auto-derived):
#                                          1. primary
#                                          2. primary-hover
#                                          3. primary-light (very light tint)
#                                          4. secondary
#                                          5. bg-secondary  (page bg tint)
#   brand.txt    (optional)            → single line with the brand name;
#                                        informational only (logged on stdout).
#
# Any image that doesn't match one of the role prefixes above is ignored with
# a warning so you can drop reference art / mockups in the same folder without
# breaking the run.
#
# Usage:
#   ./apply_ui_theme.sh              # full run: patch CSS, copy logo, rebuild
#   ./apply_ui_theme.sh --dry-run    # show intended changes, touch nothing
#   ./apply_ui_theme.sh --no-rebuild # skip `npm run build`
#   ./apply_ui_theme.sh --no-restart # skip systemctl restart apolo-frontend
#   ./apply_ui_theme.sh --source-dir=/path/to/dir
#
# Idempotent. On first run backs up frontend/src/index.css as index.css.orig
# so you can revert with `cp index.css.orig index.css`.
#===============================================================================

set -euo pipefail

# --- Defaults ----------------------------------------------------------------
APOLO_DIR="${APOLO_DIR:-/opt/ApoloBilling}"
SRC_DIR="${APOLO_DIR}/UX-UI"
FRONTEND_DIR="${APOLO_DIR}/frontend"
CSS_FILE="${FRONTEND_DIR}/src/index.css"
LOGO_DEST="${FRONTEND_DIR}/public/logo.png"

DRY_RUN=false
DO_REBUILD=true
DO_RESTART=true

for arg in "$@"; do
    case "$arg" in
        --dry-run)       DRY_RUN=true ;;
        --no-rebuild)    DO_REBUILD=false ;;
        --no-restart)    DO_RESTART=false ;;
        --source-dir=*)  SRC_DIR="${arg#*=}" ;;
        -h|--help)
            sed -n '2,35p' "$0" | sed 's/^# \{0,1\}//'
            exit 0
            ;;
        *)
            echo "Unknown flag: $arg" >&2
            exit 1
            ;;
    esac
done

# --- Colors for log output ---------------------------------------------------
if [[ -t 1 ]]; then
    GREEN='\033[0;32m'; YELLOW='\033[1;33m'; BLUE='\033[0;34m'
    RED='\033[0;31m'; NC='\033[0m'
else
    GREEN=''; YELLOW=''; BLUE=''; RED=''; NC=''
fi
log()  { printf "${BLUE}[info]${NC} %s\n" "$*"; }
warn() { printf "${YELLOW}[warn]${NC} %s\n" "$*"; }
err()  { printf "${RED}[err]${NC}  %s\n" "$*" >&2; }
ok()   { printf "${GREEN}[ok]${NC}   %s\n" "$*"; }

# --- Preflight ---------------------------------------------------------------
if [[ ! -d "$SRC_DIR" ]]; then
    err "Source dir not found: $SRC_DIR"
    exit 1
fi
if [[ ! -f "$CSS_FILE" ]]; then
    err "index.css not found at $CSS_FILE — is this the right frontend tree?"
    exit 1
fi
if ! command -v python3 >/dev/null; then
    err "python3 is required"
    exit 1
fi
if ! python3 -c "from PIL import Image" 2>/dev/null; then
    err "python3-pil (PIL) missing. Install with: apt-get install -y python3-pil"
    exit 1
fi

# --- Locate input files ------------------------------------------------------
# Role-based prefix matching (case-insensitive). First match wins per role.
shopt -s nullglob nocaseglob
LOGO_SRC=""
PALETTE_SRC=""
FAVICON_SRC=""
declare -a UNMATCHED=()

# Walk every image file once and classify it.
for f in "$SRC_DIR"/*.{png,jpg,jpeg,webp,svg,ico}; do
    [[ -f "$f" ]] || continue
    base="${f##*/}"
    lc="${base,,}"
    case "$lc" in
        logo*)                  [[ -z "$LOGO_SRC"    ]] && LOGO_SRC="$f"    ;;
        paleta*|palette*)       [[ -z "$PALETTE_SRC" ]] && PALETTE_SRC="$f" ;;
        favicon*)               [[ -z "$FAVICON_SRC" ]] && FAVICON_SRC="$f" ;;
        *)                      UNMATCHED+=("$f") ;;
    esac
done

# If there's no logo* file but only one image total (excluding favicon),
# adopt that single image as the logo. Keeps the "just drop one file" UX.
if [[ -z "$LOGO_SRC" ]]; then
    only=()
    [[ -n "$PALETTE_SRC" ]] && only+=("$PALETTE_SRC")
    only+=("${UNMATCHED[@]}")
    if [[ ${#only[@]} -eq 1 ]]; then
        LOGO_SRC="${only[0]}"
        # If we consumed the palette image as logo, don't double-use it as the
        # palette source unless it was actually named paleta*/palette*.
        UNMATCHED=()
    fi
fi

if [[ -z "$LOGO_SRC" ]]; then
    err "No logo image found in $SRC_DIR."
    err "Name one file logo*.<png|svg|jpg|jpeg|webp> (e.g. logo-white.png)."
    exit 1
fi

# Any remaining unmatched images are reference art — mention, don't fail.
for u in "${UNMATCHED[@]}"; do
    warn "Ignoring $(basename "$u") (doesn't match logo*/paleta*/favicon*)."
done

PALETTE_FILE=""
[[ -f "$SRC_DIR/palette.txt" ]] && PALETTE_FILE="$SRC_DIR/palette.txt"

BRAND_NAME=""
if [[ -f "$SRC_DIR/brand.txt" ]]; then
    BRAND_NAME="$(head -n1 "$SRC_DIR/brand.txt" | tr -d '\r\n')"
fi

shopt -u nullglob nocaseglob

log "Logo source:     $LOGO_SRC"
[[ -n "$PALETTE_SRC"  ]] && log "Palette image:   $PALETTE_SRC"
[[ -n "$FAVICON_SRC"  ]] && log "Favicon source:  $FAVICON_SRC"
[[ -n "$PALETTE_FILE" ]] && log "Palette file:    $PALETTE_FILE"
[[ -n "$BRAND_NAME"   ]] && log "Brand name:      $BRAND_NAME"

# The palette comes from PALETTE_SRC when the user provided a dedicated
# swatch, otherwise we reuse the logo pixels.
PALETTE_IMG="${PALETTE_SRC:-$LOGO_SRC}"

# --- Compute palette (Python does the heavy lifting) -------------------------
# Output format: one "KEY=#RRGGBB" per line. Keys are CSS variable names
# (without the leading `--`). Keeps the surface simple for the sed step.
readarray -t PALETTE_LINES < <(python3 - "$PALETTE_IMG" "${PALETTE_FILE:-}" <<'PYEOF'
import sys, os, colorsys
from collections import Counter
from PIL import Image

logo_path = sys.argv[1]  # really the palette source; may be the logo itself
palette_path = sys.argv[2] if len(sys.argv) > 2 and sys.argv[2] else None


def hex_to_rgb(s):
    s = s.strip().lstrip("#")
    if len(s) == 3:
        s = "".join(c * 2 for c in s)
    return tuple(int(s[i:i + 2], 16) for i in (0, 2, 4))


def rgb_to_hex(rgb):
    r, g, b = (max(0, min(255, int(round(c)))) for c in rgb)
    return f"#{r:02X}{g:02X}{b:02X}"


def rgb_to_hsl(rgb):
    r, g, b = (c / 255.0 for c in rgb)
    h, l, s = colorsys.rgb_to_hls(r, g, b)
    return h, s, l


def hsl_to_rgb(h, s, l):
    r, g, b = colorsys.hls_to_rgb(h, l, s)
    return (r * 255, g * 255, b * 255)


def adjust(hex_color, dl=0.0, ds=0.0):
    """Shift lightness by dl (-1..1) and saturation by ds."""
    h, s, l = rgb_to_hsl(hex_to_rgb(hex_color))
    l = max(0.0, min(1.0, l + dl))
    s = max(0.0, min(1.0, s + ds))
    return rgb_to_hex(hsl_to_rgb(h, s, l))


def mix(hex_color, other="#FFFFFF", ratio=0.9):
    """Blend hex_color with `other` (default white) at `ratio` of other."""
    a = hex_to_rgb(hex_color)
    b = hex_to_rgb(other)
    return rgb_to_hex(tuple(a[i] * (1 - ratio) + b[i] * ratio for i in range(3)))


def dominant_colors(path, k=8):
    """Return a list of (hex, count) sorted by brand-worthiness.

    Strategy: quantize to 32 colors, drop near-white/near-black/near-gray
    (low saturation OR extreme lightness), then rank remaining by saturation
    weighted by occurrence. Falls back to the top-count non-neutral if the
    saturated set is empty.
    """
    img = Image.open(path).convert("RGBA")
    # Flatten transparency against white so the bg doesn't leak as a color.
    bg = Image.new("RGBA", img.size, (255, 255, 255, 255))
    img = Image.alpha_composite(bg, img).convert("RGB")
    # Resize for speed; large enough to preserve brand color regions.
    img.thumbnail((200, 200))
    q = img.quantize(colors=32, method=Image.Quantize.MEDIANCUT)
    palette = q.getpalette()[:32 * 3]
    counts = Counter(q.getdata())

    scored = []
    for idx, count in counts.most_common():
        rgb = tuple(palette[idx * 3:idx * 3 + 3])
        h, s, l = rgb_to_hsl(rgb)
        # Skip neutrals (near-white, near-black, near-gray)
        if l < 0.08 or l > 0.94:
            continue
        if s < 0.15:
            continue
        # Score favors saturation and moderate lightness; count tie-breaks.
        score = s * 2 + (1 - abs(l - 0.5)) + (count / 1e6)
        scored.append((score, count, rgb_to_hex(rgb)))

    scored.sort(reverse=True)
    if not scored:
        # Extreme fallback: take the most common non-pure-white color.
        for idx, count in counts.most_common():
            rgb = tuple(palette[idx * 3:idx * 3 + 3])
            if rgb != (255, 255, 255):
                return [rgb_to_hex(rgb)]
        return ["#0099D9"]  # ultimate fallback
    return [hex_ for _, _, hex_ in scored[:k]]


# --- Resolve primary palette -------------------------------------------------
# Precedence: palette.txt > derived from image
OVERRIDES = []
if palette_path and os.path.exists(palette_path):
    with open(palette_path) as f:
        for line in f:
            line = line.strip()
            if not line or line.startswith("#") and len(line) > 7:
                # Skip comment-style lines (# comment, not # hex)
                continue
            if line.startswith("#"):
                OVERRIDES.append(line.upper())

palette = dominant_colors(logo_path)

primary = OVERRIDES[0] if len(OVERRIDES) > 0 else palette[0]
hover   = OVERRIDES[1] if len(OVERRIDES) > 1 else adjust(primary, dl=-0.12)
light   = OVERRIDES[2] if len(OVERRIDES) > 2 else mix(primary, "#FFFFFF", 0.88)
secondary_default = palette[1] if len(palette) > 1 else adjust(primary, dl=0.0, ds=-0.4)
secondary = OVERRIDES[3] if len(OVERRIDES) > 3 else secondary_default
bg2       = OVERRIDES[4] if len(OVERRIDES) > 4 else mix(primary, "#FFFFFF", 0.96)

# Derived ramps for the rest of the design system.
primary_dark = adjust(primary, dl=-0.18)
border_primary   = mix(primary, "#FFFFFF", 0.78)
border_secondary = mix(primary, "#FFFFFF", 0.60)
bg_tertiary      = mix(primary, "#FFFFFF", 0.92)
bg_hover         = mix(primary, "#FFFFFF", 0.90)

# Sidebar stays dark but picks up the primary as accent.
sidebar_active = mix(primary, "#FFFFFF", 0.45)

# Shadow tint — primary with low alpha
r, g, b = hex_to_rgb(primary)
shadow    = f"rgba({r}, {g}, {b}, 0.08)"
shadow_md = f"rgba({r}, {g}, {b}, 0.12)"
selection_bg = mix(primary, "#FFFFFF", 0.78)
selection_fg = adjust(primary, dl=-0.28)

# info badge tracks primary; success/warning/error stay semantic.
info_bg = mix(primary, "#FFFFFF", 0.92)

out = {
    "color-brand-blue": primary,
    "color-brand-blue-dark": primary_dark,
    "color-brand-blue-light": adjust(primary, dl=0.10),
    "color-primary": primary,
    "color-primary-hover": hover,
    "color-primary-light": light,
    "color-secondary": secondary,
    "color-bg-secondary": bg2,
    "color-bg-tertiary": bg_tertiary,
    "color-bg-hover": bg_hover,
    "color-border-primary": border_primary,
    "color-border-secondary": border_secondary,
    "color-shadow": shadow,
    "color-shadow-md": shadow_md,
    "color-info": primary,
    "color-info-bg": info_bg,
    "color-scrollbar-track": bg_tertiary,
    "color-scrollbar-thumb": border_secondary,
    "color-scrollbar-thumb-hover": primary,
    "color-sidebar-text-active": sidebar_active,
    "color-sidebar-accent": primary,
    # Not CSS vars but still useful in the glow/selection rules further down:
    "_selection-bg": selection_bg,
    "_selection-fg": selection_fg,
}
for k, v in out.items():
    print(f"{k}={v}")
PYEOF
)

# Extract key=value pairs into an associative array.
declare -A P
for line in "${PALETTE_LINES[@]}"; do
    [[ -z "$line" ]] && continue
    P["${line%%=*}"]="${line#*=}"
done

log "Extracted palette:"
printf "  primary        %s\n" "${P[color-primary]}"
printf "  primary-hover  %s\n" "${P[color-primary-hover]}"
printf "  primary-light  %s\n" "${P[color-primary-light]}"
printf "  secondary      %s\n" "${P[color-secondary]}"
printf "  bg-secondary   %s\n" "${P[color-bg-secondary]}"

# --- Dry-run short-circuit ---------------------------------------------------
if $DRY_RUN; then
    warn "Dry run — no files written, no rebuild, no restart."
    exit 0
fi

# --- Backup + patch index.css ------------------------------------------------
if [[ ! -f "${CSS_FILE}.orig" ]]; then
    cp -a "$CSS_FILE" "${CSS_FILE}.orig"
    log "Original CSS backed up as ${CSS_FILE}.orig"
fi

# Apply each key/value as a sed replacement against the :root block.
# Use '|' delimiter because hex/rgba strings contain no '|'.
TMP=$(mktemp)
cp -a "$CSS_FILE" "$TMP"

for key in "${!P[@]}"; do
    [[ "$key" == _* ]] && continue   # private helpers, not CSS vars
    val="${P[$key]}"
    # Replace the VALUE on lines like `  --color-primary: #0099D9;`
    sed -i -E "s|(--${key}:)[[:space:]]*[^;]+;|\1 ${val};|" "$TMP"
done

# Rewrite hardcoded ::selection colors that bypass CSS vars
sed -i -E \
    -e "s|(background-color:)[[:space:]]*#[0-9A-Fa-f]{6};([[:space:]]*/\* selection bg \*/)?|\1 ${P[_selection-bg]};|" \
    "$TMP" 2>/dev/null || true

# Replace hardcoded rgba(0, 153, 217, ...) fibertel blue occurrences with the
# new primary's rgba so the animations/btn-primary keep on-brand.
R="$(python3 -c "
h='${P[color-primary]}'.lstrip('#')
print(','.join(str(int(h[i:i+2],16)) for i in (0,2,4)))
")"
sed -i -E "s|rgba\(0,[[:space:]]*153,[[:space:]]*217,|rgba(${R},|g" "$TMP"
# Replace the linear-gradient that hardcodes the primary/primary-dark pair.
sed -i -E \
    -e "s|#0099D9|${P[color-primary]}|gi" \
    -e "s|#007BB5|${P[color-primary-hover]}|gi" \
    -e "s|#33ADDF|${P[color-brand-blue-light]}|gi" \
    -e "s|#38BDF8|${P[color-sidebar-text-active]}|gi" \
    "$TMP"

mv "$TMP" "$CSS_FILE"
ok "index.css patched"

# --- Copy logo + favicon -----------------------------------------------------
# We always write frontend/public/logo.png, re-encoding if the source was SVG
# or a different format. Keep the original extension for favicon.
python3 - "$LOGO_SRC" "$LOGO_DEST" <<'PYEOF'
import sys, shutil, os
from PIL import Image
src, dst = sys.argv[1], sys.argv[2]
ext = os.path.splitext(src)[1].lower()
if ext == ".png":
    shutil.copyfile(src, dst)
else:
    Image.open(src).convert("RGBA").save(dst, "PNG")
print(f"logo -> {dst}")
PYEOF
ok "Logo copied to $LOGO_DEST"

if [[ -n "$FAVICON_SRC" ]]; then
    fav_ext="${FAVICON_SRC##*.}"
    fav_dst="${FRONTEND_DIR}/public/favicon.${fav_ext,,}"
    cp -a "$FAVICON_SRC" "$fav_dst"
    ok "Favicon copied to $fav_dst"
fi

# --- Rebuild frontend --------------------------------------------------------
if $DO_REBUILD; then
    log "Running npm run build (this takes ~30s)..."
    ( cd "$FRONTEND_DIR" && npm run build >/tmp/apolo_ui_build.log 2>&1 ) \
        && ok "Frontend rebuilt" \
        || { err "npm run build failed; see /tmp/apolo_ui_build.log"; exit 1; }
else
    warn "Skipping npm run build (--no-rebuild). Run it manually to see changes."
fi

# --- Restart service so the compiled bundle is served ------------------------
if $DO_RESTART && $DO_REBUILD; then
    if systemctl list-unit-files apolo-frontend.service >/dev/null 2>&1; then
        systemctl restart apolo-frontend && ok "apolo-frontend restarted"
    fi
fi

ok "Theme applied. Reload the browser (hard refresh: Ctrl+Shift+R) to see it."
