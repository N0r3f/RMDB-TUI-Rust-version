# RMDB - Relative Manchot Diskless Boot

Interface en ligne de commande (TUI) pour monitorer et gérer le serveur IPXE RMDB. Permet également de déployer RMDB dans un container LXC Alpine Linux sur n'importe quelle distribution Linux.

## Installation Rapide

### Méthode 1 : Script de lancement (recommandé)

```bash
# Lancer directement (compile si nécessaire)
./run.sh

# Ou en mode release (optimisé)
./run.sh release

# Forcer la recompilation (utile si les modifications ne sont pas détectées)
./run.sh --force-rebuild
# ou
./run.sh release --force-rebuild
```

### Méthode 2 : Installation système

```bash
# Installation pour l'utilisateur (~/.local/bin)
./install.sh --user

# Installation système (nécessite sudo)
sudo ./install.sh --system
```

### Méthode 3 : Makefile

```bash
# Compiler et lancer
make run

# Compiler en mode release
make release

# Installer pour l'utilisateur
make install

# Installer système (nécessite sudo)
sudo make install-sys

# Voir toutes les commandes
make help
```

### Méthode 4 : Compilation manuelle

```bash
# Compiler le projet
cargo build --release

# Exécuter
./target/release/rmdb
```

## Utilisation

1. Lancez l'application avec `./run.sh` ou `rmdb`
2. Choisissez un mode d'exécution (1=Lecture seule, 2=Safe, 3=Admin)
3. Naviguez avec les flèches haut/bas
4. Sélectionnez avec Entrée
5. Quittez avec Q

## Structure des Menus

- **Services** : Gestion des services RMDB (DHCP, DNS, TFTP, HTTP)
- **IPXE** : Gestion du menu iPXE
- **Clients** : Visualisation des clients et leases DHCP
- **VMs** : Gestion des machines virtuelles
- **Configuration** : Configuration du serveur
- **Monitoring** : Logs et métriques
- **Système** : Informations système et déploiement LXC
- **Déploiement** : Création de container LXC Alpine pour installer RMDB

## Prérequis

- Rust 1.70+ (installé via [rustup.rs](https://rustup.rs/))
- Terminal d'au moins 80x24 caractères
- Accès au serveur RMDB (pour les fonctionnalités complètes)

Voir `.ai-core/README.md` pour la documentation complète.

