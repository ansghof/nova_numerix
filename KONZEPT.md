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

Ein Level dauert etwa 2–3 Minuten (2 Minuten Flug plus Boss): dichte, abwechslungsreiche
Gegnerwellen (Schlange, V-Formation, Zange, Sturzflug, Kreisel), Checkpoint, Boss. Ein Fortschrittsbalken oben in der Mitte zeigt, wie weit
es noch bis zum Boss ist, mit einer Markierung für den Checkpoint. Er läuft in Spielzeit,
steht während einer Aufgabe also fast still.

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
- Etwa jede dritte Aufgabe ist eine **Lückenaufgabe** („3 × _ = 15“, Antwort 5), mit einer
  Sekunde mehr Zeit. Das gilt auch für die Zahlenplatten des Bosses.
- Zu leichte Fakten (z. B. ×10 im kleinen 1×1) kommen anfangs nur mit einem Viertel der
  normalen Häufigkeit.

### Ausgelöschte Wellen
Wird eine Gegnerwelle vollständig abgeschossen, ohne dass einer entkommt, gibt es eine
goldene Doppel-Druckwelle, den Schriftzug „Welle ausgelöscht!“ und eine Bonusaufgabe:
richtig = Sonderpunkte (60 je Gegner, mit Tempo bis zum Dreifachen), Zeit um = kein Bonus,
falsch = kein Bonus und leichter Schildschaden. Frachtdrohnen und Superwaffe haben Vorrang;
es werden höchstens zwei Bonusaufgaben aufgehoben.

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

### Bosskampf (umgesetzt am Beispiel „Der Wächter“)
Vor dem Kern des Bosses sitzen vier Zahlenplatten. Oben steht eine Aufgabe; wer die Platte
mit dem richtigen Ergebnis abschießt, öffnet den Kern für 6 Sekunden. Die falschen Zahlen
sind plausibel (Nachbarn, Zehnersprünge, vertauschte Ziffern). Hier läuft keine Zeitlupe:
Rechnen ist Zielen, das Zeitlimit ist deshalb 1,6-mal so lang wie im Aufgabenfenster.

| Ergebnis | Folge |
|---|---|
| richtige Platte, schnell | Kern 6 s offen, Punkte mit Tempo-Bonus |
| Zeit abgelaufen | Lösung leuchtet grün, Kern nur 3 s offen, keine Punkte |
| falsche Platte | Platte blinkt rot, Lösung wird gezeigt, Kern bleibt zu, gezielter Feuerstoß |

Ab halber Energie feuert der Boss schneller. Die Superwaffe trifft ihn mit vierfachem
Schaden, auch wenn der Kern zu ist. Nach dem Sieg zeigt eine Auswertung Punkte und
Aufgabenbilanz; die Waffen bleiben an Bord.

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

**Rechentraining:** Thema frei wählbar, Kombo-Multiplikator für Serien richtiger Antworten.
Mit etwa **20–30 gelösten Aufgaben** soll man sich im Shop vernünftig versorgen können.
Weiterrechnen ist immer möglich, lohnt sich aber pro Werft-Besuch immer weniger, damit man
nach dem ersten Level nicht sofort übermächtig ist:

| Aufgabe im Besuch | Credits pro richtiger Antwort |
|---|---|
| 1–25 | 100 % (Grundwert ~40 Credits, plus Tempo-Bonus) |
| 26–40 | 50 % |
| 41–60 | 25 % |
| ab 61 | 10 % (Üben bleibt möglich, bringt aber kaum noch etwas) |

Nach jedem geflogenen Level (auch einem verlorenen) beginnt die Staffel wieder von vorn.
Die Anzeige zeigt offen, wie viele „volle“ Aufgaben noch übrig sind, damit sich niemand
betrogen fühlt.

**Knappes Budget:** Nach einem Level plus vollem Training reicht es für **ein bis zwei
Dinge**. Wer beschädigt zurückkommt, muss erst reparieren und hat dann kaum noch etwas übrig.
Jeder Kauf soll eine echte Entscheidung sein.

**Richtwerte für die Preise** (Level-Credits + voller Trainingsbesuch ≈ 700–900 Credits):
Reparatur 10 Credits je Schildpunkt (eine halbe Reparatur ≈ 500), neue Waffe 600,
Waffenstufe 400 × Stufe, Panzerung 700, Zeitlupen-Chip 200, Superwaffen-Ladung 350,
neuer Rumpf 3000–8000 (also über viele Besuche sparen).

**Shop:** Reparatur, Waffen, Panzerung, Feuerrate, zusätzliche Superwaffen-Ladung (max. +2),
Zeitlupen-Chips (+50 % Zeit für X Aufgaben), Rümpfe, Lackierungen.

**Verhandeln mit Quibbo, dem Rechenkrämer:**

| Ergebnis | Rabatt |
|---|---|
| richtig und schnell | bis 10 % |
| richtig, aber langsam | 2–3 % |
| Zeit abgelaufen, aber richtig | 1 % |
| falsch | 0 %, Quibbo ist für diesen Artikel beleidigt, bis man den Shop verlässt |

### Umgesetzt (Stand Werft)
- Credits im Flug: 2–6 je Abschuss, 10 je richtiger Aufgabe, 150 für den Boss. Sie gehen
  erst nach dem Sieg aufs Konto; beim Tod sind die Flug-Credits weg.
- Training: 12 Credits + bis 8 Tempo-Bonus je richtiger Aufgabe, Staffel 100/50/25/10 %,
  falsch = 3 s Denkpause, Zeit um = Lösung wird gezeigt.
- Shop: Reparatur (+25 Schild für 250), Laser-Stufe (400 × aktuelle Stufe), Raketenwerfer
  600 bzw. Raketen-Stufe 400 × neue Stufe, Superwaffen-Ladung 350 (max. 2), drei
  Zeitlupen-Chips 200. Verhandeln (V) einmal je Artikel und Besuch: richtig 3–10 %,
  Zeit um 1 %, falsch 0 % und Quibbo redet über den Artikel nicht mehr. Ein Rabatt gilt
  für einen Kauf.
- Der Schild bleibt zwischen den Leveln, wie er ist; Reparatur kostet.
- Beim Betreten des Ladens erklärt Quibbo (als Neon-Figur neben dem Ladentisch), wie Credits,
  Kaufen und Verhandeln funktionieren. Er freut sich über Käufe und gute Rechnungen und läuft
  rot an, wenn man ihn mit einer falschen Antwort beleidigt.
- Speichern: 5 Slots, automatisch in der Werft (Ankunft, jeder Kauf, Training, Abflug).
  Gespeichert werden Schiffsname, Credits, Waffen, Schild, gekaufte Extras, freigeschaltete
  Level, Punkte und der Lernstand je Thema. Wer im Flug beendet, startet beim Laden wieder
  in der Werft vor diesem Flug. Dateien liegen unter %APPDATA%/NovaNumerix.
- Noch offen: Panzerung, neue Rümpfe.

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
