# Nova Numerix – Spielkonzept

Ein seitlich scrollender Weltraum-Shooter (links → rechts) für 10- bis 15-Jährige, in dem
Kopfrechnen die stärkste Waffe ist. Gebaut mit Bevy (Rust), Grafik im Neon-Vektor-Stil.
Plattform: PC mit Tastatur, später optional Web (WASM).

## Spielablauf

```
Hauptmenü → Speicherslot (1–5) → Sternenkarte (Levelauswahl)
   → Werft: Rechentraining für Credits → Shop (mit Verhandeln)
   → Level: Flug, Wellen, Aufgaben-Popups, Superwaffe, Checkpoints
   → Bosskampf → Auswertung (Punkte, Sterne, Statistik) → Sternenkarte
```

Ein Level dauert etwa 4–6 Minuten: Anflug, 3–4 Abschnitte mit Gegnerwellen, ein
Zwischenereignis, Checkpoint, Boss.

## Steuerung

| Profil | Linke Hand | Rechte Hand |
|---|---|---|
| **Standard** | WASD fliegen, Leertaste feuern, Q Superwaffe | Nummernblock rechnen, Numpad-Enter bestätigen |
| **Laptop** | – | Pfeiltasten fliegen, Ziffernreihe rechnen, Enter bestätigen |

Im Standardprofil muss niemand umgreifen. Trotzdem folgt jeder Aufgabe eine kurze
**Rückkehr-Phase (~0,7 s)**: Ergebnis anzeigen, die Zeit beschleunigt sanft von 10 % auf
100 %, das Schiff blinkt und ist bis 1 s danach unverwundbar.

## Aufgaben

Grundregel: **falsch < Zeit abgelaufen < richtig < richtig und schnell**.

| Ergebnis | Belohnung | Rückmeldung |
|---|---|---|
| Richtig und schnell | volle Belohnung + Tempo-Bonus (fällt mit der Zeit ab) | „Blitzschnell!“ |
| Richtig, aber knapp | volle Belohnung, kleiner Bonus | „Sauber gerechnet!“ |
| Zeit abgelaufen | Powerup ja, Punkte nein | „Geschafft! Beim nächsten Mal schneller.“ + Lösung |
| Falsch | nichts, leichter Schildschaden (nie tödlich) | „Knapp daneben: 7 × 8 = 56“ |

- Nach Fehler oder Timeout wird die Lösung immer groß gezeigt.
- Jedes Rechenfakt (z. B. 7×8) hat einen Lernwert; schwache Fakten kommen öfter wieder.
- Zeitlimits passen sich an (schnelle Spieler bekommen etwas weniger Zeit).
- Startwerte: kleines 1×1 6 s, großes 1×1 10 s.

### Bullet-Time
Während einer Aufgabe läuft die Welt mit 10 % weiter. Das Schiff liegt in einer
**Schutzblase**: Schüsse prallen ab, Gegner werden weggeschoben, Gegner auf Kollisionskurs
beim Öffnen zur Seite gedrückt. Niemand stirbt, weil er gerade rechnet.

### Frachtdrohnen
Gold leuchtende Frachtdrohnen fliegen zwischen den Wellen. Abschuss → Aufgaben-Popup →
Powerup.

### Superwaffe (3 pro Level)
Schaden = Minimum + Restzeit-Anteil × (Maximum − Minimum), mit drei sichtbaren Stufen
(Schockwelle / große Explosion / kleiner Puls). Bei falscher Antwort kommt sofort eine
**zweite Aufgabe**, der Balken läuft weiter. Ist auch sie falsch oder läuft die Zeit ab: Minimal-Schuss.
Gegen Bosse 3- bis 5-facher Schaden.

### Mathe im Kampf
- **Zahlen-Formationen:** Drei Gegner tragen Zahlen, oben steht eine Aufgabe. Den richtigen
  abschießen → Kombo-Bonus.
- **Boss-Schwachstellen:** Panzerplatten mit Zahlen, nur die richtige öffnet sich.

## Waffen und Ausbau

Primärwaffen feuern gleichzeitig, jede mit Stufe 1–5. 2× Laser = Laser Stufe 2,
Raketen dazu = Laser + Raketen.

| Waffe | Verhalten |
|---|---|
| Laser | gerade, schnell |
| Streuschuss | Fächer nach vorn |
| Raketen | zielsuchend |
| Plasmawelle | durchdringt Gegner |
| Blitzbogen | springt zwischen Gegnern |
| Heckkanone | schießt nach hinten |

Anbauteile sind am Schiff sichtbar: Waffenplattformen, Drohnen, Schildgenerator,
Panzerplatten, Nachbrenner.

| Rumpf | Plattformen |
|---|---|
| Spatz | 2 (Start) |
| Falke | 3 |
| Kondor | 4 |
| Phönix | 5 |

### Tod und Fortschritt (Roguelite)

| Geht beim Tod verloren | Bleibt immer |
|---|---|
| alle Waffen und Waffenstufen (aufgesammelt und gekauft) | Rumpf, Plattform-Anzahl |
| Drohnen, Level-Powerups | gekaufte Grundwerte (Panzerung, Feuerrate, Schild) |
| im laufenden Level gesammelte Credits | freigeschaltete Level, Sterne, Lernstatistik |

Checkpoints im Level (z. B. vor dem Boss). Eine „Serie“-Anzeige zählt Level ohne Tod und
gibt Bonus-Credits.

## Werft und Shop

**Rechentraining:** 60-Sekunden-Runde, Kombo-Multiplikator, Thema frei wählbar, keine
Obergrenze. Credits pro Aufgabe etwas niedriger als im Level.

**Shop:** Reparatur, Waffen, Panzerung, Feuerrate, zusätzliche Superwaffen-Ladung (max. +2),
Zeitlupen-Chips (+50 % Zeit für X Aufgaben), Rümpfe, Lackierungen.

**Verhandeln mit Quibbo, dem Rechenkrämer:**

| Ergebnis | Rabatt |
|---|---|
| richtig und schnell | bis 10 % |
| richtig, aber langsam | 2–3 % |
| Zeit abgelaufen, aber richtig | 1 % |
| falsch | 0 %, Quibbo ist für diesen Artikel beleidigt, bis man den Shop verlässt |

## Level

| # | Thema | Welt (Akzentfarbe) | Boss |
|---|---|---|---|
| 1 | Addition bis 20 | Asteroidengürtel (Bernstein) | Der Schrottsammler – Greifarme mit Zahlenplatten |
| 2 | Subtraktion bis 20 | Eisnebel (Eisblau) | Frostwurm – Segmente der Reihe nach abschießen |
| 3 | Kleines 1×1 | Pilzwälder (Giftgrün) | Sporenmutter – Kapseln mit Produkten |
| 4 | Quadratzahlen | Gitterstation (Violett) | Der Kubus – quadratische Panzerplatten |
| 5 | Großes 1×1 | Gasriese (Sturmorange) | Sturmfestung – Mehrphasen-Boss |
| 6 | Binär 4 Bit | Datenkern (Matrixgrün) | Der Prozessor – Bit-Lampen als Schwachstellen |

**Binäreingabe:** vier Kippschalter (8, 4, 2, 1), umschalten mit den Tasten 1–4.

Pro Level 3 Sterne (geschafft / über 80 % richtig / Boss mit Superwaffe Stufe 3 besiegt),
Schwierigkeitsgrade Kadett / Pilot / Ass.

Später denkbar: Verdoppeln und Halbieren, Division, Mischlevel, Endlosmodus.

## Speicherstände

5 Slots. Name des Slots = Schiffsname + Zeitstempel, z. B. „Sternfalke · 30.10.2026 14:32“.
Der Schiffsname (max. 20 Zeichen) taucht auch in Funksprüchen auf.
Gespeichert werden Schiff, Credits, freigeschaltete Level, Sterne und die Lernstatistik pro
Fakt (für eine Lernübersicht).

## Grafik: Neon-Vektor

Dunkler Hintergrund, alles Wichtige leuchtet (Linien mit HDR-Farben + Bloom).

| Element | Farbe |
|---|---|
| Spieler und eigene Schüsse | Cyan / Weiß |
| Gegner | Magenta / Rot |
| Gegnerschüsse | Orange mit hellem Kern |
| Frachtdrohnen, Powerups | Gold |
| Boss-Schwachstellen | pulsierendes Grün |

Hintergrund in drei Parallax-Ebenen. Explosionen aus auseinanderfliegenden, nachglühenden
Linienfragmenten.

## Technik

- Bevy 0.20, Zustände `AppState`, im Level SubStates (`Flying`, `TaskPopup`, …).
- Aufgaben über ein Trait `TaskGenerator`; neues Thema = neue Implementierung.
- Level als RON-Dateien mit Zeitleiste der Wellen.
- Vektorgrafik über Gizmos (Linien) mit HDR-Kamera und Bloom; Partikel als eigene,
  schlanke CPU-Simulation (läuft auch im Browser).
- Speichern hinter einem Trait (`SaveStorage`): Datei auf dem PC, später `localStorage` im Web.
- Nur Deutsch.

### WASM vorbereiten
- Kein direkter Dateizugriff außerhalb des Speicher-Traits.
- Keine GPU-Partikel-Crates, die WebGPU voraussetzen.
- Startbildschirm mit Tastendruck/Klick, damit Audio im Browser starten darf.
