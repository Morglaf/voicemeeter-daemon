# Voicemeeter Controller

Daemon Windows léger qui contrôle Voicemeeter (Banana / Potato) via les touches multimédia du clavier.

## Prérequis

- Windows 10/11 (x64)
- [Voicemeeter Banana ou Potato](https://vb-audio.com/Voicemeeter/banana.htm) installé et lancé
- Rust (pour compiler localement) : [rustup.rs](https://rustup.rs)

## Compilation

```powershell
cargo build --release
```

Le binaire se trouve dans `target\release\voicemeeter-controller.exe`.

Compilez sur votre machine pour limiter les faux positifs de Windows Defender (le binaire intercepte des entrées clavier globales).

## Lancement au démarrage

1. Compiler en release (voir ci-dessus)
2. Copier `voicemeeter-controller.exe` où vous voulez (ex. `C:\Tools\voicemeeter-controller\`)
3. Ouvrir le dossier de démarrage : `Win+R` → `shell:startup` → Entrée
4. Créer un raccourci vers l'exécutable dans ce dossier

## Raccourcis par défaut

| Touche | Action |
|--------|--------|
| **Volume +** | Augmente le gain du bus actif (+2 dB par défaut) |
| **Volume −** | Diminue le gain du bus actif |
| **Mute** | Passe au bus suivant dans la liste switchable |

## Comportement du switch

- Le **volume** s'applique uniquement au bus actif
- Tous les **autres canaux switchables sont mutés** automatiquement (ex. volume sur A3 → A1 est muté)
- La touche **Mute** fait défiler le bus actif parmi les canaux configurés (A1 → A3 → A1…)

Par défaut les canaux switchables sont **A1** et **A3**.

## Configuration

Fichier : `%APPDATA%\voicemeeter-controller\config.json`

```json
{
  "switch_buses": [0, 2],
  "active_bus": 0,
  "volume_increment": 2.0,
  "hotkeys": {
    "switch": "AudioVolumeMute",
    "volume_up": "AudioVolumeUp",
    "volume_down": "AudioVolumeDown"
  }
}
```

- `switch_buses` : liste des bus participant au switch (0 = A1, 1 = A2, 2 = A3…)
- `active_bus` : bus qui reçoit les changements de volume (les autres sont mutés)
- `volume_increment` : pas en dB (0.5, 1.0, 2.0 ou 5.0 via le menu tray)
- `hotkeys` : noms des touches (`AudioVolumeUp`, `AudioVolumeDown`, `AudioVolumeMute`) ou combos (`Ctrl+Alt+KeyS`)

### Migration legacy

Si un fichier `vm_target.txt` (contenant l'index du bus) est placé à côté de l'exécutable au premier lancement, sa valeur initialise `active_bus` puis est migrée vers `config.json`.

## Icône personnalisée

Placez un fichier `icon.ico` (ou `app.ico`) dans le **même dossier** que l'exécutable pour remplacer l'icône bleue par défaut. La plus grande résolution du fichier `.ico` est utilisée.

## Icône system tray

Clic droit sur l'icône dans la barre des tâches :

- Cocher/décocher les **canaux switchables** (A1 à A5, minimum 2)
- Choisir le **bus actif** (volume)
- Régler l'incrément de volume
- Reconnecter Voicemeeter (si redémarré)
- Quitter

## Dépannage

| Problème | Solution |
|----------|----------|
| L'app ne démarre pas | Vérifier que Voicemeeter est ouvert |
| Les touches multimédia ne réagissent pas | Une autre app peut les capturer ; modifier les hotkeys dans `config.json` |
| La touche Mute mute aussi Windows | Comportement Windows ; rebinder `switch` sur un combo clavier dans la config |
| Defender bloque l'exécutable | Compiler localement en `--release` et ajouter une exclusion si nécessaire |

## Structure du projet

```
src/
  main.rs      — boucle d'événements winit
  config.rs    — persistance JSON
  vm.rs        — API Voicemeeter
  actions.rs   — swap mute et volume
  hotkeys.rs   — raccourcis globaux
  tray.rs      — icône et menu system tray
```
