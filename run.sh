#!/bin/sh
# Lanceur simple pour RMDB
# Usage: ./run.sh [mode] [--force-rebuild|-f]
#   mode: debug (défaut) ou release
#   --force-rebuild, -f: Force la recompilation même si le binaire est à jour

# Ne pas utiliser set -e car certaines fonctions retournent des codes d'erreur non-critiques
SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
MODE="debug"
FORCE_REBUILD=false

# Parser les arguments
for arg in "$@"; do
    case "$arg" in
        debug|release)
            MODE="$arg"
            ;;
        --force-rebuild|-f)
            FORCE_REBUILD=true
            ;;
        *)
            # Si c'est le premier argument et que ce n'est pas une option, c'est le mode
            if [ "$MODE" = "debug" ] && [ "$arg" != "--force-rebuild" ] && [ "$arg" != "-f" ]; then
                MODE="$arg"
            fi
            ;;
    esac
done

# Couleurs pour les messages
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
RED='\033[0;31m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

log() {
    echo -e "${BLUE}[$(date +'%H:%M:%S')]${NC} $1"
}

ok() {
    echo -e "${GREEN}[OK]${NC} $1"
}

warn() {
    echo -e "${YELLOW}[WARN]${NC} $1"
}

error() {
    echo -e "${RED}[ERROR]${NC} $1"
}

# Détecter la distribution Linux
detect_distribution() {
    if [ -f /etc/os-release ]; then
        . /etc/os-release
        echo "$ID"
    elif [ -f /etc/debian_version ]; then
        echo "debian"
    elif [ -f /etc/redhat-release ]; then
        echo "rhel"
    elif [ -f /etc/arch-release ]; then
        echo "arch"
    else
        echo "unknown"
    fi
}

# Installer Rust/Cargo automatiquement
install_rust() {
    log "Installation de Rust/Cargo..."
    
    if [ -f "$HOME/.cargo/env" ]; then
        . "$HOME/.cargo/env"
        if command -v cargo >/dev/null 2>&1; then
            ok "Rust/Cargo déjà installé (via rustup)"
            return 0
        fi
    fi
    
    # Télécharger et installer rustup
    log "Téléchargement de rustup..."
    curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y
    
    if [ $? -ne 0 ]; then
        error "Échec de l'installation de Rust"
        return 1
    fi
    
    # Charger l'environnement Rust
    if [ -f "$HOME/.cargo/env" ]; then
        . "$HOME/.cargo/env"
        ok "Rust/Cargo installé avec succès"
        return 0
    else
        error "Rust installé mais .cargo/env introuvable"
        return 1
    fi
}

# Installer les dépendances de compilation selon la distribution
install_build_dependencies() {
    DISTRO=$(detect_distribution)
    
    log "Détection de la distribution: $DISTRO"
    
    # Vérifier si les outils sont déjà installés
    if command -v gcc >/dev/null 2>&1 && command -v make >/dev/null 2>&1 && command -v curl >/dev/null 2>&1; then
        ok "Dépendances de compilation déjà installées"
        return 0
    fi
    
    log "Installation des dépendances de compilation..."
    
    case "$DISTRO" in
        debian|ubuntu|linuxmint|kali)
            if ! command -v gcc >/dev/null 2>&1 || ! command -v make >/dev/null 2>&1 || ! command -v curl >/dev/null 2>&1; then
                log "Installation de build-essential et curl..."
                if command -v sudo >/dev/null 2>&1; then
                    if sudo apt-get update -qq && sudo apt-get install -y build-essential curl; then
                        ok "Dépendances installées avec succès"
                    else
                        warn "Échec de l'installation automatique"
                        warn "Installez manuellement: sudo apt-get install build-essential curl"
                        return 1
                    fi
                else
                    warn "sudo non disponible, installation manuelle requise: apt-get install build-essential curl"
                    return 1
                fi
            fi
            ;;
        fedora|rhel|centos|rocky|almalinux)
            if ! command -v gcc >/dev/null 2>&1 || ! command -v make >/dev/null 2>&1 || ! command -v curl >/dev/null 2>&1; then
                log "Installation de gcc, make et curl..."
                if command -v sudo >/dev/null 2>&1; then
                    if command -v dnf >/dev/null 2>&1; then
                        if sudo dnf install -y gcc make curl; then
                            ok "Dépendances installées avec succès"
                        else
                            warn "Échec de l'installation automatique"
                            return 1
                        fi
                    elif command -v yum >/dev/null 2>&1; then
                        if sudo yum install -y gcc make curl; then
                            ok "Dépendances installées avec succès"
                        else
                            warn "Échec de l'installation automatique"
                            return 1
                        fi
                    fi
                else
                    warn "sudo non disponible, installation manuelle requise"
                    return 1
                fi
            fi
            ;;
        arch|manjaro|endeavouros)
            if ! command -v gcc >/dev/null 2>&1 || ! command -v make >/dev/null 2>&1 || ! command -v curl >/dev/null 2>&1; then
                log "Installation de base-devel et curl..."
                if command -v sudo >/dev/null 2>&1; then
                    if sudo pacman -Sy --noconfirm base-devel curl; then
                        ok "Dépendances installées avec succès"
                    else
                        warn "Échec de l'installation automatique"
                        warn "Installez manuellement: sudo pacman -S base-devel curl"
                        return 1
                    fi
                else
                    warn "sudo non disponible, installation manuelle requise: pacman -S base-devel curl"
                    return 1
                fi
            fi
            ;;
        opensuse|sles)
            if ! command -v gcc >/dev/null 2>&1 || ! command -v make >/dev/null 2>&1 || ! command -v curl >/dev/null 2>&1; then
                log "Installation de gcc, make et curl..."
                if command -v sudo >/dev/null 2>&1; then
                    if sudo zypper install -y gcc make curl; then
                        ok "Dépendances installées avec succès"
                    else
                        warn "Échec de l'installation automatique"
                        return 1
                    fi
                else
                    warn "sudo non disponible, installation manuelle requise"
                    return 1
                fi
            fi
            ;;
        alpine)
            if ! command -v gcc >/dev/null 2>&1 || ! command -v make >/dev/null 2>&1 || ! command -v curl >/dev/null 2>&1; then
                log "Installation de build-base et curl..."
                if command -v sudo >/dev/null 2>&1; then
                    if sudo apk add --no-cache build-base curl; then
                        ok "Dépendances installées avec succès"
                    else
                        warn "Échec de l'installation automatique"
                        return 1
                    fi
                else
                    if apk add --no-cache build-base curl; then
                        ok "Dépendances installées avec succès"
                    else
                        warn "Échec de l'installation automatique"
                        return 1
                    fi
                fi
            fi
            ;;
        *)
            warn "Distribution non reconnue: $DISTRO"
            warn "Assurez-vous d'avoir installé: gcc, make, curl"
            if ! command -v gcc >/dev/null 2>&1 || ! command -v make >/dev/null 2>&1 || ! command -v curl >/dev/null 2>&1; then
                return 1
            fi
            ;;
    esac
    
    ok "Dépendances de compilation vérifiées"
    return 0
}

# Vérifier et installer Rust si nécessaire
check_and_install_rust() {
    if command -v cargo >/dev/null 2>&1; then
        ok "Rust/Cargo est déjà installé"
        return 0
    fi
    
    warn "Rust/Cargo n'est pas installé."
    log "Installation automatique de Rust/Cargo..."
    
    # Vérifier que curl est disponible
    if ! command -v curl >/dev/null 2>&1; then
        error "curl n'est pas installé. Installation de curl..."
        install_build_dependencies
    fi
    
    # Installer Rust
    if install_rust; then
        ok "Rust/Cargo installé avec succès"
        return 0
    else
        error "Échec de l'installation automatique de Rust"
        error "Installez Rust manuellement depuis https://rustup.rs/"
        exit 1
    fi
}

# Vérifier et installer les dépendances
log "Vérification des dépendances..."

# Installer les dépendances de compilation si nécessaire
if ! install_build_dependencies; then
    warn "Certaines dépendances n'ont pas pu être installées automatiquement"
    warn "Le script continuera mais la compilation peut échouer"
fi

# Vérifier et installer Rust si nécessaire
check_and_install_rust

# S'assurer que cargo est dans le PATH
if [ -f "$HOME/.cargo/env" ]; then
    . "$HOME/.cargo/env"
fi

# Corriger l'erreur dump_bash_state si elle existe
if ! type dump_bash_state >/dev/null 2>&1; then
    dump_bash_state() { :; }
    export -f dump_bash_state 2>/dev/null || true
fi

# Vérification finale
if ! command -v cargo >/dev/null 2>&1; then
    error "Rust/Cargo n'est toujours pas disponible après l'installation"
    error "Essayez de relancer le script ou installez Rust manuellement: curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh"
    exit 1
fi

log "Vérification du projet..."

# Vérifier que Cargo.toml existe
if [ ! -f "$SCRIPT_DIR/Cargo.toml" ]; then
    error "Cargo.toml introuvable dans $SCRIPT_DIR"
    exit 1
fi

# Déterminer le chemin du binaire
if [ "$MODE" = "release" ]; then
    BINARY="$SCRIPT_DIR/target/release/rmdb"
    BUILD_MODE="--release"
else
    BINARY="$SCRIPT_DIR/target/debug/rmdb"
    BUILD_MODE=""
fi

# Fonction pour vérifier si une recompilation est nécessaire
needs_rebuild() {
    # Si le binaire n'existe pas, il faut compiler
    if [ ! -f "$BINARY" ]; then
        return 0  # true - besoin de compiler
    fi
    
    # Vérifier si Cargo.toml est plus récent
    if [ "$SCRIPT_DIR/Cargo.toml" -nt "$BINARY" ]; then
        return 0  # true - besoin de compiler
    fi
    
    # Vérifier si Cargo.lock est plus récent
    if [ -f "$SCRIPT_DIR/Cargo.lock" ] && [ "$SCRIPT_DIR/Cargo.lock" -nt "$BINARY" ]; then
        return 0  # true - besoin de compiler
    fi
    
    # Vérifier si un fichier source est plus récent que le binaire
    # Utiliser find pour trouver tous les fichiers .rs et vérifier leur date
    if find "$SCRIPT_DIR/src" -name "*.rs" -newer "$BINARY" 2>/dev/null | grep -q .; then
        return 0  # true - besoin de compiler
    fi
    
    # Vérifier si le dossier src est plus récent (pour les nouveaux fichiers)
    if [ "$SCRIPT_DIR/src" -nt "$BINARY" ]; then
        return 0  # true - besoin de compiler
    fi
    
    # Si aucune modification détectée, pas besoin de recompiler
    return 1  # false - pas besoin de compiler
}

# Compiler si nécessaire
if [ "$FORCE_REBUILD" = true ] || needs_rebuild; then
    if [ "$FORCE_REBUILD" = true ]; then
        log "Recompilation forcée..."
    fi
    log "Compilation en mode $MODE..."
    if [ "$MODE" = "release" ]; then
        cargo build --release
    else
        cargo build
    fi
    
    if [ $? -ne 0 ]; then
        error "Échec de la compilation"
        exit 1
    fi
    ok "Compilation réussie"
else
    log "Binaire à jour, pas de compilation nécessaire"
fi

# Vérifier que le binaire existe
if [ ! -f "$BINARY" ]; then
    error "Binaire introuvable: $BINARY"
    error "Essayez de compiler manuellement: cargo build $BUILD_MODE"
    exit 1
fi

# Vérifier la taille minimale du terminal
log "Vérification de la taille du terminal..."
COLS=$(tput cols 2>/dev/null || echo 80)
LINES=$(tput lines 2>/dev/null || echo 24)

if [ "$COLS" -lt 80 ] || [ "$LINES" -lt 24 ]; then
    warn "Terminal trop petit: ${COLS}x${LINES}"
    warn "Taille minimale recommandée: 80x24"
    warn "Le TUI peut ne pas s'afficher correctement"
    read -p "Continuer quand même ? (o/N) " -n 1 -r
    echo
    if [[ ! $REPLY =~ ^[OoYy]$ ]]; then
        log "Annulé par l'utilisateur"
        exit 0
    fi
fi

# Lancer le TUI
log "Lancement de RMDB..."
log "Mode: $MODE"
log "Taille terminal: ${COLS}x${LINES}"
echo ""

# Exécuter le binaire
exec "$BINARY"

