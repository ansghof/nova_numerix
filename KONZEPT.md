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
Powerup. Was sie geladen haben, steht schon beim Anflug fest: Ein Zeichen in der Mitte und
ein Schild darunter zeigen es (Laser, Raketen, Reparatur, Seitenkanone, Drohne). So kann man
gezielt jagen, was man braucht.

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

| Rumpf | Waffenplätze | Schild | Preis | Besonderheit |
|---|---|---|---|---|
| Spatz | 2 | 100 | Start | |
| Falke | 3 | 120 | 3000 | Entenflügel |
| Kondor | 4 | 140 | 5000 | Doppelflügel |
| Phönix | 4 | 160 | 8000 | Schildgenerator: lädt im Flug 0,4 Schild je Sekunde nach |

Ein Waffenplatz trägt ein Waffensystem: Laser (immer an Bord), Raketen, Seitenkanonen,
Drohnen. Ausbauen geht immer, ein neues System braucht einen freien Platz. Auf dem Spatz
heißt das: Laser plus eine weitere Waffe, also eine echte Entscheidung. Frachtdrohnen bringen
keine neuen Systeme, wenn kein Platz frei ist; wird es unterwegs eng, gibt es stattdessen eine
Reparatur.

**Panzerung** (3 Stufen, 700/1400/2100): jede Stufe hält 15 % jedes Treffers durch Schüsse und
Zusammenstöße ab. Der Schildabzug bei falschen Antworten bleibt gleich. Die Platten liegen als
Kranz um den Rumpf, ein Ring je Stufe. Rumpf und Panzerung bleiben beim Verlust des Schiffs.

### Tod und Fortschritt (Roguelite)

| Geht beim Tod verloren | Bleibt immer |
|---|---|
| alle Waffen und Waffenstufen (aufgesammelt und gekauft) | Rumpf, Plattform-Anzahl |
| Drohnen, Level-Powerups | gekaufte Grundwerte (Panzerung, Feuerrate, Schild) |
| im laufenden Level gesammelte Credits | freigeschaltete Level, Sterne, Lernstatistik |

Checkpoints im Level (umgesetzt): einer bei 60 % (Markierung im Fortschrittsbalken, wird grün)
und einer direkt vor dem Boss. Erreicht man einen, wandern die bis dahin gesammelten
Flug-Credits aufs Konto und sind sicher. Geht das Schiff verloren, startet ein neues am letzten
Checkpoint: Das Feld wird geräumt, ein Boss kommt mit voller Kraft neu herein. Verloren sind
die Waffen und die seit dem Checkpoint gesammelten Credits; Superwaffen-Ladungen, Punkte und
Aufgabenbilanz bleiben. Eine Meldung am unteren Rand sagt jeweils, was passiert ist. Eine „Serie“-Anzeige zählt Level ohne Tod und
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
- Neue Ware mit jedem besiegten Boss; bis dahin steht sie mit Hinweis im Laden:
  nach Boss 1 Seitenkanonen (500, dann 450 × neue Stufe, max. 3: schräg nach oben und
  unten, gut gegen Geschütze am Rand), nach Boss 2 Begleitdrohnen (900 und 1800, max. 2:
  fliegen über und unter dem Schiff mit und feuern mit). Frachtdrohnen bringen die neuen
  Waffen ebenfalls, sobald Quibbo sie führt; Drohnen nur selten.
- Bedienung in Menü und Werft auch mit der Maus: Zeigen wählt, Klicken bestätigt.
- Rümpfe und Panzerung siehe oben. Im Laden: Panzerung und nächster Rumpf; Waffen ohne
  freien Platz stehen mit Hinweis da. Beim Verhandeln zeigt der Laden nur die Ware, um die es
  geht.

### Umgesetzt (Gegner je Level)
- Neonnebel: Wackler-Schlange, Pfeile in V-Formation, Zange, Sturzflug, Kreisel-Ring.
- Gitterstation: Geschütze, die auf dem Gitter an Boden und Decke vorbeiziehen und
  Dreiergarben feuern; Teiler, die beim Abschuss in zwei Splitter zerfallen (die Welle ist erst
  mit den Splittern ausgelöscht); Sucher, die auf die Höhe des Schiffs nachsteuern, bis es
  vorbei ist. Dazu selten Pfeile und Kreisel.

## Level

| # | Thema | Welt (Akzentfarbe) | Boss |
|---|---|---|---|
| 1 | Addition bis 20 | Asteroidengürtel (Bernstein) | Der Schrottsammler – Greifarme mit Zahlenplatten |
| 2 | Subtraktion bis 20 | Eisnebel (Eisblau) | Frostwurm – Segmente der Reihe nach abschießen |
| 3 | Kleines 1×1 | Pilzwälder (Giftgrün) | Sporenmutter – Kapseln mit Produkten |
| 4 | Quadratzahlen | Gitterstation (Violett) | Der Kubus – quadratische Panzerplatten |
| 5 | Großes 1×1 | Gasriese (Sturmorange) | Sturmfestung – Mehrphasen-Boss |
| 6 | Binär 4 Bit | Datenkern (Matrixgrün) | Der Prozessor – Bit-Lampen als Schwachstellen |

**Binäreingabe (umgesetzt):** Die Antwort wird mit 0 und 1 auf dem Nummernblock getippt;
führende Nullen darf man weglassen, angezeigt wird sie vierstellig. Unter jeder Binäraufgabe
stehen die Stellenwerte 8 · 4 · 2 · 1.

### Umgesetzt (Level)
| # | Welt | Thema | Boss | Eigene Gegner |
|---|---|---|---|---|
| 1 | Neonnebel | Kleines 1×1 | Der Wächter | Wackler, Pfeile, Kreisel |
| 2 | Gitterstation | Quadratzahlen | Der Kubus | Geschütze, Teiler, Sucher |
| 3 | Gasriese | Großes 1×1 (11–20 × 2–9) | Die Sturmfestung: ab halber Kraft zusätzlich drehende Schussringe | Minen (gehen hoch, wenn man zu nahe kommt; dann kein Abschuss und keine Wellenprämie), Kreuzer mit Fünferfächer |
| 4 | Datenkern | Binärzahlen (4 Bit, beide Richtungen) | Der Prozessor: Platten zeigen Binärzahlen, falsche unterscheiden sich in einem Bit | Bits (nur als 1 verwundbar, kippen alle 1,4 s um), Datenströme |

Hintergründe: am Gasriesen ein gestreifter Planet, im Datenkern herabrieselnde Datenströme.
Addition und Subtraktion bis 20 sind für 10- bis 15-Jährige zu leicht und entfallen vorerst.

Pro Level 3 Sterne (umgesetzt): geschafft / mindestens 80 % richtig / den Boss mit voller
Superwaffen-Ladung (Stufe 3) erledigt. Der Siegerbildschirm zeigt alle drei mit Erklärung und
einen Tipp für den fehlenden; gespeichert wird der Bestwert je Level. Die Werft zeigt die Sterne
beim Level und die Summe, die Slot-Liste ebenfalls.

Schwierigkeitsgrade (umgesetzt), beim neuen Spiel mit ←/→ gewählt und in der Werft änderbar:

| Grad | Zeit je Aufgabe | Schaden durch Treffer | Abstand der Wellen |
|---|---|---|---|
| Kadett | × 1,4 | × 0,6 | × 1,3 |
| Pilot | × 1,0 | × 1,0 | × 1,0 |
| Ass | × 0,8 | × 1,3 | × 0,8 |

Der Schildabzug bei falschen Antworten ist auf allen Graden gleich. Sterne gibt es auf jedem
Grad gleich, damit niemand auf Kadett benachteiligt ist.

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

## Ton (umgesetzt)

Alle Geräusche und die Musik entstehen beim Start im Code (Rechteck, Dreieck, Sägezahn,
Rauschen), es gibt keine Tondateien. Die Musikstücke werden im Hintergrund erzeugt.

- Musik: ruhiges Stück für Menü und Werft, je Level ein eigenes (a-Moll, d-Moll, e-Moll,
  cis-Moll, unterschiedlich schnell), ein schnelleres im Bosskampf. Wechsel werden
  überblendet; während der Boss explodiert, ist Stille, dann folgt eine kleine Siegesfanfare.
  Während einer Aufgabe wird die Musik leiser.
- Geräusche: Laser, Raketen, Explosionen, Treffer am Schiff, Schiffsverlust, Aufgabe öffnet,
  richtig / blitzschnell / falsch (bewusst sanft) / Zeit um, Superwaffe, ausgelöschte Welle,
  Checkpoint, Boss-Alarm, Klicks in Menü und Werft, Kauf.
- M schaltet die Musik, N die Geräusche an und aus (nicht im Menü, dort wird getippt).
- Zum Anhören schreibt `cargo test` alle Klänge nach `target/sounds`.

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
