#!/usr/bin/env bash
# ============================================================================
# update-voxtype.sh — Helper script for safely updating Voxtype
#
# Usage:
#   ./scripts/update-voxtype.sh
#
# What it does:
#   1. Shows current version
#   2. Checks latest version on GitHub
#   3. Creates a backup of the current binary
#   4. Shows download instructions
#   5. After manual install, validates the new version
#   6. Offers rollback if validation fails
#
# This script does NOT auto-download or auto-install.
# It assists the user with a safe, reversible workflow.
# ============================================================================

set -uo pipefail

# Colors
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
BOLD='\033[1m'
NC='\033[0m' # No Color

BINARY_PATH=""

# --------------------------------------------------------------------------
# Helpers
# --------------------------------------------------------------------------

info()    { echo -e "${BLUE}ℹ${NC} $*"; }
success() { echo -e "${GREEN}✓${NC} $*"; }
warn()    { echo -e "${YELLOW}⚠${NC} $*"; }
error()   { echo -e "${RED}✗${NC} $*"; }

find_binary() {
    BINARY_PATH="$(command -v voxtype 2>/dev/null || true)"
    if [[ -z "$BINARY_PATH" ]]; then
        # Common locations
        for candidate in "$HOME/.local/bin/voxtype" "/usr/local/bin/voxtype" "/usr/bin/voxtype"; do
            if [[ -x "$candidate" ]]; then
                BINARY_PATH="$candidate"
                return
            fi
        done
        error "Impossible de trouver le binaire voxtype."
        error "Installez-le d'abord ou ajoutez-le au PATH."
        exit 1
    fi
}

# --------------------------------------------------------------------------
# Step 1: Current version
# --------------------------------------------------------------------------

step_current_version() {
    echo ""
    echo -e "${BOLD}═══════════════════════════════════════════════════${NC}"
    echo -e "${BOLD}  Mise à jour de Voxtype${NC}"
    echo -e "${BOLD}═══════════════════════════════════════════════════${NC}"
    echo ""

    find_binary
    local current
    current="$(voxtype --version 2>/dev/null | awk '{print $NF}')"

    if [[ -z "$current" ]]; then
        error "Impossible de déterminer la version courante."
        exit 1
    fi

    info "Binaire : ${BOLD}$BINARY_PATH${NC}"
    info "Version courante : ${BOLD}v$current${NC}"
    echo ""

    echo "$current"
}

# --------------------------------------------------------------------------
# Step 2: Check latest version
# --------------------------------------------------------------------------

step_check_latest() {
    local current="$1"

    info "Vérification de la dernière version sur GitHub…"

    local json
    json="$(curl -sfL --max-time 15 \
        "https://api.github.com/repos/peteonrails/voxtype/releases/latest" 2>/dev/null)"

    if [[ -z "$json" ]]; then
        error "Impossible de contacter l'API GitHub."
        error "Vérifiez votre connexion réseau."
        exit 1
    fi

    local tag
    tag="$(echo "$json" | grep -o '"tag_name"[[:space:]]*:[[:space:]]*"[^"]*"' | head -1 | cut -d'"' -f4)"
    local latest="${tag#v}"

    local url
    url="$(echo "$json" | grep -o '"html_url"[[:space:]]*:[[:space:]]*"[^"]*"' | head -1 | cut -d'"' -f4)"

    if [[ -z "$latest" ]]; then
        error "Impossible de parser la réponse GitHub."
        exit 1
    fi

    info "Dernière version : ${BOLD}v$latest${NC}"

    if [[ "$current" == "$latest" ]]; then
        success "Vous êtes déjà à jour ! (v$current)"
        exit 0
    fi

    echo ""
    success "Mise à jour disponible : v$current → ${GREEN}${BOLD}v$latest${NC}"
    echo -e "  Changelog : ${BLUE}$url${NC}"
    echo ""

    echo "$latest"
}

# --------------------------------------------------------------------------
# Step 3: Backup
# --------------------------------------------------------------------------

step_backup() {
    local backup_path="${BINARY_PATH}.bak"

    info "Création d'une sauvegarde…"

    if cp "$BINARY_PATH" "$backup_path" 2>/dev/null; then
        success "Backup créé : ${BOLD}$backup_path${NC}"
    else
        warn "Impossible de créer le backup (permissions ?)"
        warn "Essayez : sudo cp $BINARY_PATH $backup_path"
        read -rp "Continuer sans backup ? [o/N] " choice
        if [[ "$choice" != "o" && "$choice" != "O" ]]; then
            info "Abandon."
            exit 0
        fi
    fi
}

# --------------------------------------------------------------------------
# Step 4: Download instructions
# --------------------------------------------------------------------------

step_download_instructions() {
    local latest="$1"

    echo ""
    echo -e "${BOLD}────────────────────────────────────────────────────${NC}"
    echo -e "${BOLD}  Instructions de mise à jour${NC}"
    echo -e "${BOLD}────────────────────────────────────────────────────${NC}"
    echo ""
    echo "  Téléchargez la nouvelle version depuis :"
    echo ""
    echo -e "    ${BLUE}https://github.com/peteonrails/voxtype/releases/tag/v${latest}${NC}"
    echo ""
    echo "  Puis remplacez le binaire :"
    echo ""
    echo -e "    ${YELLOW}# Arrêter le daemon${NC}"
    echo "    systemctl --user stop voxtype"
    echo ""
    echo -e "    ${YELLOW}# Copier le nouveau binaire${NC}"
    echo "    cp ~/Téléchargements/voxtype $BINARY_PATH"
    echo "    chmod +x $BINARY_PATH"
    echo ""
    echo -e "    ${YELLOW}# Redémarrer le daemon${NC}"
    echo "    systemctl --user start voxtype"
    echo ""
    echo -e "${BOLD}────────────────────────────────────────────────────${NC}"
    echo ""

    read -rp "Appuyez sur Entrée une fois la mise à jour effectuée… "
}

# --------------------------------------------------------------------------
# Step 5: Validation
# --------------------------------------------------------------------------

step_validate() {
    local expected="$1"

    echo ""
    info "Validation de la mise à jour…"
    echo ""

    # Check version
    local new_version
    new_version="$(voxtype --version 2>/dev/null | awk '{print $NF}')"

    if [[ "$new_version" == "$expected" ]]; then
        success "Version vérifiée : v$new_version"
    else
        error "Version inattendue : v$new_version (attendu : v$expected)"
        step_rollback
        return
    fi

    # Check dependencies
    if command -v voxtype &>/dev/null; then
        info "Vérification des dépendances…"
        if voxtype setup check 2>/dev/null; then
            success "Toutes les dépendances sont OK."
        else
            warn "Certaines dépendances posent problème."
            warn "Exécutez 'voxtype setup' pour les corriger."
        fi
    fi

    echo ""
    echo -e "${BOLD}═══════════════════════════════════════════════════${NC}"
    echo -e "  ${GREEN}${BOLD}✓ Mise à jour réussie !${NC} Voxtype v$new_version"
    echo -e "${BOLD}═══════════════════════════════════════════════════${NC}"
    echo ""

    # Clean up backup
    local backup_path="${BINARY_PATH}.bak"
    if [[ -f "$backup_path" ]]; then
        read -rp "Supprimer le backup ($backup_path) ? [O/n] " choice
        if [[ "$choice" != "n" && "$choice" != "N" ]]; then
            rm -f "$backup_path"
            success "Backup supprimé."
        else
            info "Backup conservé : $backup_path"
        fi
    fi
}

# --------------------------------------------------------------------------
# Step 6: Rollback (if validation fails)
# --------------------------------------------------------------------------

step_rollback() {
    local backup_path="${BINARY_PATH}.bak"

    if [[ ! -f "$backup_path" ]]; then
        error "Pas de backup disponible pour la restauration."
        error "Vous devrez réinstaller manuellement."
        return
    fi

    echo ""
    warn "La validation a échoué."
    read -rp "Restaurer la version précédente depuis le backup ? [O/n] " choice
    if [[ "$choice" == "n" || "$choice" == "N" ]]; then
        info "Rollback annulé. Le backup est conservé : $backup_path"
        return
    fi

    info "Arrêt du daemon…"
    systemctl --user stop voxtype 2>/dev/null || true

    if cp "$backup_path" "$BINARY_PATH" 2>/dev/null; then
        success "Version précédente restaurée."
        info "Redémarrage du daemon…"
        systemctl --user start voxtype 2>/dev/null || true
        success "Rollback terminé."
    else
        error "Échec de la restauration. Essayez manuellement :"
        error "  cp $backup_path $BINARY_PATH"
    fi
}

# --------------------------------------------------------------------------
# Main
# --------------------------------------------------------------------------

main() {
    local current latest

    current="$(step_current_version)"
    latest="$(step_check_latest "$current")"
    step_backup
    step_download_instructions "$latest"
    step_validate "$latest"
}

main "$@"
