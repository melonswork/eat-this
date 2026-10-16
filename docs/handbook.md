# Jak zdekompilować starą grę — podręcznik z projektu „Eat This”

Na przykładzie gry Eat This (1998, Win16, Borland Pascal 7) przeniesionej do Rusta + SDL2.

## 1. Zanim zaczniesz

- **Wyciągnij dane z oryginalnego nośnika** i trzymaj je poza repo (`work/`). Wszystkie
  narzędzia instaluj powtarzalnie (tu: mise — Ghidra, Python/uv, Rust).
- **Rozpoznaj format pliku wykonywalnego i kompilator.** Win16 NE + Borland Pascal oznacza:
  segmenty `1000:xxxx`, konwencja Pascala (argumenty odkładane od lewej), obiekty z VMT w słowie
  0, typ `Real` 6-bajtowy, generator `Random` jako LCG (RandSeed × 0x08088405 + 1).
- **Ghidra** zrobi pierwszą dekompilację całości. Nadawaj nazwy w pliku tekstowym
  (`re/names.csv`) i nakładaj je skryptem — wtedy nazwy przeżyją ponowną analizę.

## 2. Najważniejsza decyzja: wyrocznia

Bez obiektywnego testu nie wiesz, czy port jest wierny. Tutaj wyrocznią są **nagrane dema**:
gra zapisuje w nich co 35 klatek stan generatora losowego (CHECK). Jeśli port w każdym oknie
wykonuje te same wywołania `Random()`, to — bo prawie każda decyzja gry zależy od losowania —
symuluje tę samą grę. Szukaj w swojej grze podobnej rzeczy: dema, powtórki, sumy kontrolne,
zapisy stanu.

## 3. Uruchom oryginał pod emulatorem i patrz na niego

Najcenniejsze narzędzie projektu to **tracer**: oryginalny EXE uruchomiony w emulatorze CPU
(unicorn) z minimalną atrapą Windowsa — bez maszyny wirtualnej. Daje:

- listę każdego wywołania `Random()` z klatką i wołającym,
- wartości zmiennych globalnych na początku każdej klatki (`--watch`),
- kto i kiedy zapisał daną komórkę pamięci (`--writes`) — to rozstrzyga większość zagadek
  („kto zmienił tę ścianę?”),
- wejścia do funkcji z łańcuchem wołających i zrzutem pól obiektu (`--entry`, `--self`).

Porównujesz to z identycznym wyjściem z portu (`--dump`, `--trace`) i szukasz pierwszej
różnicy. Pierwsza różnica jest ważna; wszystko po niej to zwykle jej skutki.

## 4. Pętla pracy: okno po oknie

1. Uruchom port na demie → „pierwsza rozbieżność w oknie N”.
2. Zobacz w tracerze, które funkcje losowały w tym oknie, a w porcie — które nie.
3. Jeśli to nowa klasa/efekt: przeczytaj dekompilację, przepisz pole po polu.
4. Jeśli to subtelna różnica: porównaj stan bohatera/obiektu klatka po klatce, znajdź pierwszą
   różniącą się wartość, potem `--writes` na niej.
5. Upewnij się, że wcześniejsze dema dalej przechodzą (regresje są realne). Commit.

Postęp mierzy się liczbą zgodnych okien — w tym projekcie od 7 do ~107 okien CITY01 w kilka
dni, a demo GRINDER przechodzi w całości.

## 5. Czytanie dekompilacji — pułapki

- **Ghidra gubi argumenty** wywołań pomocników arytmetycznych (dzielenie/mnożenie long,
  przesunięcia). Gdzie widzisz `FUN_1168_0fdd()` bez argumentów, czytaj asembler — inaczej
  zgadujesz wzór.
- **Kolejność parametrów**: Pascal odkłada od lewej, więc „param_1” Ghidry to ostatni argument
  źródłowy. Sprawdzaj na znanych funkcjach (np. `(y, x)`), zanim zaufasz.
- **Procedury zagnieżdżone** czytają zmienne lokalne rodzica przez łącze statyczne
  (`param_1 - 0xe`). Ghidra potrafi skleić dwie różne zmienne w jedną — sprawdź offsety w asm.
- **Zmienne bajtowe** obok siebie czytane jako słowo dają dziwne wartości (257 = dwa bajty po 1).
- **Arytmetyka**: odtwarzaj dokładnie szerokość (16 bit z zawijaniem), znak, zaokrąglanie
  (dzielenie obcina do zera; x87 zaokrągla do parzystej).
- To, co wygląda na grafikę, potrafi sterować logiką: klatka sprite'a wyznacza hitbox, licznik
  animacji wyznacza moment strzału.

## 6. Jak przepisywać

- **Pola obiektów jako surowa pamięć** (`o.w(0x28)`), nie od razu ładne struktury. Dzięki temu
  kod portu da się czytać obok dekompilacji linijka w linijkę; ładne nazwy daj w komentarzu.
- **Zachowaj kolejność** wszystkiego, co losuje lub zmienia wspólny stan — także wewnątrz klatki
  (np. bohater przed obiektami, cząstki grupy 0 przed obiektami, grupy 1 po nich).
- **Kosmetykę możesz pominąć**, ale tylko jeśli udowodnisz, że nie wpływa na logikę (dźwięk,
  rysowanie). Cząsteczki dekoracyjne bywają zdradliwe: mogą trafiać, odbijać się, losować.
- **Nieznane gałęzie ostrzegają** (`warning: … not ported`), zamiast milczeć — ostrzeżenia to
  lista zadań.

## 7. Czego się nauczyliśmy

- Lepiej zbudować narzędzie (tracer, porównywarka) niż długo zgadywać. Każde narzędzie zwróciło
  się wielokrotnie.
- Duże klasy przeciwników przepisuj w całości, a nie tylko gałąź potrzebną teraz — i tak wrócisz.
- Nie wierz, że okno „zgadza się”, jeśli liczby losowań się zgadzają: różnice mogą się znosić
  w obrębie okna. Porównuj też klatki.
- Zapisuj odkrycia od razu (`re/*.md`) — kontekst się kończy, wiedza zostaje.
