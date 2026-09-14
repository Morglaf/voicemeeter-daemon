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

## Mini-clavier HID (Zoom / Video)

Sous-menu **Mini-clavier** dans le tray :

1. **Activer** — capture exclusive du périphérique HID (bloque Vol/Mute système sur ce clavier)
2. **Périphérique** — choisir le device dans la liste (défaut `1189:8890`)
3. **Mappings** — pour chaque bouton, configurer l'action en mode Zoom et Video
4. **Réinitialiser mappings** — restaurer la matrice par défaut

### Modes

- **Zoom** (défaut) : navigation / design (Photoshop, etc.)
- **Video** : contrôle pas-à-pas

Le **knob clic (Mute)** bascule entre Zoom et Video (toast + tooltip). Les événements volume du mini-clavier **ne passent pas** à Windows.

### Matrice par défaut

| Entrée | Mode Zoom | Mode Video |
|--------|-----------|------------|
| Bouton 1 (Prev) | Ctrl+Alt+* | Ctrl+Alt+* |
| Bouton 2 (Next) | Ctrl+Alt+" | Ctrl+Alt+" |
| Bouton 3 (Play) | Ctrl+Alt+) | Ctrl+Alt+) |
| Knob clic | Bascule → Video | Bascule → Zoom |
| Knob gauche | Ctrl+Molette bas | Flèche gauche |
| Knob droite | Ctrl+Molette haut | Flèche droite |

Les combos utilisent le layout clavier US (SendInput). Sur AZERTY, le caractère produit peut différer visuellement.

La config est sauvegardée automatiquement dans `config.json` (section `mini_keyboard`).

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
| La touche Mute mute aussi Windows | Comportement Windows sur le **clavier principal** ; le mini-clavier HID est isolé si activé |
| Mini-clavier : open exclusif échoue | Débrancher/rebrancher ; fermer toute app qui monopolise le HID |
| Pas de toast au changement de mode | Normal si notifications Windows désactivées ; le tooltip tray affiche le mode |
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
  tray_mini_keyboard.rs — config mini-clavier via tray
  mini_keyboard/ — capture HID, mapping, injection SendInput
```
