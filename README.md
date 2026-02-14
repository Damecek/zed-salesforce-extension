# Zed Apex LSP Extension – architektura a zadání (MVP)

Tento repozitář je **analytický/architektonický start** projektu, jehož cílem je dodat rozšíření do [Zed](https://zed.dev), které přidá podporu jazyka **Salesforce Apex** přes **Language Server Protocol (LSP)**.

> Tento krok záměrně neimplementuje runtime kód rozšíření; definuje jasné technické zadání, zdroje, architekturu a plán MVP.

## 1) Cíl projektu

Vybudovat Zed extension, která:

- rozpozná Apex soubory (`.cls`, `.trigger`, případně `.apex`),
- poskytne minimální jazykovou ergonomii přes Tree-sitter (komentáře, základní tokeny, zvýraznění syntaxe),
- spustí Apex LSP server (Java proces s `apex-jorje-lsp.jar`),
- naváže standardní LSP komunikaci mezi Zed ↔ Apex serverem,
- umožní první užitečné funkce: diagnostiky, základní completion, „go to definition“ (podle schopností serveru).

## 2) Primární zdroje dokumentace

### Zed

1. Zed: Developing Extensions  
   https://zed.dev/docs/extensions/developing-extensions
2. Zed: Language Extensions  
   https://zed.dev/docs/extensions/languages
3. Zed Extension API (Rust): trait `Extension`, `language_server_command`, API pro download a status instalace  
   https://github.com/zed-industries/zed/tree/main/crates/extension_api

### Salesforce / Apex LSP

1. Oficiální Salesforce doc: Apex Language Server  
   https://developer.salesforce.com/docs/platform/sfvscode-extensions/guide/apex-language-server.html
2. Referenční implementace VS Code extension:  
   https://github.com/forcedotcom/salesforcedx-vscode
3. Konkrétní soubor spuštění LSP ve VS Code:  
   `packages/salesforcedx-vscode-apex/src/languageServer.ts`
4. Referenční JAR v repozitáři Salesforce VS Code:  
   `packages/salesforcedx-vscode-apex/jars/apex-jorje-lsp.jar`

## 3) Co víme z referencí (shrnutí)

### 3.1 Zed extension model

- Zed extension je repozitář s `extension.toml`.
- Procedurální část extension je Rust → kompilace do WebAssembly (`cdylib`, `zed_extension_api`).
- Jazyková podpora je složená z:
  - `languages/<jazyk>/config.toml` (metadata: název, suffixy, komentáře…),
  - Tree-sitter grammar registrace v `extension.toml`,
  - query soubory (`highlights.scm`, `brackets.scm`, `outline.scm`, `indents.scm`, …),
  - volitelně LSP server přes `language_servers` + `language_server_command` v Rustu.

### 3.2 Apex LSP ve VS Code (forcedotcom)

- Apex LSP se spouští jako Java proces nad JAR:
  - classpath: `apex-jorje-lsp.jar`
  - main class: `apex.jorje.lsp.ApexLanguageServerLauncher`
- VS Code implementace řeší:
  - detekci/validaci Javy (včetně minimální verze, v kódu je kontrola `>=11`),
  - skládání JVM argumentů,
  - inicializační LSP options,
  - file watchery a restart scénáře.

### 3.3 Salesforce oficiální doc

- Apex Language Server je IDE-agnostický LSP server.
- Oficiálně doporučuje použít `apex-jorje-lsp.jar` a jako příklad inicializace uvádí `languageServer.ts` ve forcedotcom repozitáři.
- Dokumentuje provozní témata (status, restart, reset indexu).

## 4) Návrh cílové architektury pro Zed

## 4.1 Komponenty

1. **Zed extension (Rust/WASM)**
   - implementuje `Extension` trait,
   - vrací command pro spuštění LSP (`language_server_command`),
   - volitelně dodává initialization/workspace config.

2. **Language metadata + Tree-sitter vrstva**
   - `languages/apex/config.toml`
   - query soubory minimálně `highlights.scm` (+ ideálně `brackets.scm`)
   - grammar registrace v `extension.toml`.

3. **Apex LSP runtime (Java + JAR)**
   - Java binárka (`java`) dostupná na host systému,
   - JAR artefakt (`apex-jorje-lsp.jar`) jako payload extension nebo stažený artefakt.

4. **Salesforce project context**
   - očekává se workspace ve stylu SFDX/`sfdx-project.json`,
   - bez něj může být část funkcí omezená (indexace/analýza).

## 4.2 Tok spuštění

1. Uživatel otevře Apex soubor v Zed.
2. Zed language mapping přiřadí soubor k `Apex` (suffix + grammar).
3. Zed spustí language server dle `extension.toml` + Rust callbacku.
4. Rust extension vrátí command:
   - `java`
   - argumenty: `-cp <path-to-jar> apex.jorje.lsp.ApexLanguageServerLauncher`
5. Proběhne LSP initialize/initialized handshake.
6. Zed začne posílat dokumentové události a přijímat diagnostiky/symboly/completion.

## 4.3 Kde vzít Java binárku

Možnosti pro MVP:

- **A) Preferovat systémovou Javu (doporučeno pro první MVP)**
  - hledat `JAVA_HOME/bin/java`, případně `java` v `PATH`,
  - při chybě zobrazit jasnou instrukci uživateli.

- **B) Uživatelská konfigurace**
  - přidat setting typu `apex.java_path`/`apex.java_home`.

- **C) Bundlovaná JRE**
  - technicky možná, ale výrazně zvětšuje distribuci a multiplatformní složitost.

Doporučení: MVP = varianta A+B.

## 4.4 Kde vzít `apex-jorje-lsp.jar`

Možnosti:

- **A) Vendoring v repozitáři extension** (nejjednodušší start)
  - přidat JAR do adresáře extension (např. `jars/`),
  - plus dokumentovat verzi a update proces.

- **B) Download při prvním spuštění** přes API extension hosta
  - použít `download_file`, řídit stav přes `set_language_server_installation_status`,
  - lepší velikost repa, složitější robustnost/cache/checksum.

Doporučení: MVP = A (rychlý start), následně přejít na B.

## 5) Funkční rozsah MVP

## 5.1 Povinné

1. **Language registration v Zed**
   - název jazyka „Apex“,
   - suffixy minimálně `.cls`, `.trigger`, volitelně `.apex`,
   - line comment (`//`).

2. **Základní zvýraznění syntaxe (Tree-sitter)**
   - komentáře,
   - stringy,
   - čísla,
   - klíčová slova,
   - identifikátory.

3. **LSP start/stop lifecycle**
   - start Java procesu,
   - schopnost reconnect/restart po pádu,
   - logování chyb čitelně v Zed logu.

4. **LSP handshake a základní features**
   - diagnostics,
   - completion,
   - definition (pokud server vrací).

5. **Dokumentace provozu**
   - požadovaná verze Java,
   - jak ověřit funkčnost,
   - známá omezení.

## 5.2 Nice-to-have po MVP

- semantic tokens (v kombinaci s Tree-sitter),
- code actions / rename / references,
- lepší project-awareness (SFDX workspace validation),
- automatické stahování/updaty JAR.

## 6) Návrh repozitářové struktury pro implementační fázi

```text
.
├─ extension.toml
├─ Cargo.toml
├─ src/
│  └─ lib.rs
├─ languages/
│  └─ apex/
│     ├─ config.toml
│     ├─ highlights.scm
│     └─ brackets.scm
├─ jars/
│  └─ apex-jorje-lsp.jar
└─ docs/
   ├─ architecture.md
   ├─ decisions.md
   └─ testing.md
```

## 7) Rozhodnutí a rizika

1. **Verze Javy**
   - Salesforce VS Code kód validuje Java runtime minimálně 11.
   - Riziko: uživatelé bez kompatibilní Javy.

2. **Licencování a redistribuce JAR**
   - před vendoringem právně ověřit podmínky redistribuce `apex-jorje-lsp.jar`.

3. **Závislost na Salesforce project layoutu**
   - některé LSP funkce mohou být slabé mimo standardní SFDX strukturu.

4. **Cross-platform runtime rozdíly**
   - Windows/macOS/Linux cesty + quoting argumentů.

## 8) Jak bude AI agent testovat práci (doporučení)

Cílem je mít co nejvíce automatizovaný smoke-test bez GUI klikání.

## 8.1 Testovací vrstvy

1. **Statická kontrola extension konfigurace**
   - validace existence `extension.toml`, `languages/apex/config.toml`, query souborů.

2. **Runtime smoke test LSP procesu**
   - shell skript spustí `java -cp jars/apex-jorje-lsp.jar apex.jorje.lsp.ApexLanguageServerLauncher` a ověří, že proces běží.

3. **LSP protocol smoke test**
   - použít nástroj typu `lsp-devtools` / JSON-RPC harness:
     - poslat `initialize`,
     - otevřít sample Apex dokument,
     - zkusit `textDocument/completion` nebo `textDocument/definition`.

4. **Integrační test v Zed dev extension režimu**
   - nainstalovat jako „Install Dev Extension“,
   - otevřít sample SFDX projekt,
   - zkontrolovat Zed log (`zed: open log`) a ověřit attach LSP serveru.

## 8.2 Doporučené test assets

- Minimalní ukázkový SFDX projekt v `fixtures/sfdx-sample/`.
- Apex soubory s:
  - komentáři (`//`, `/* */`),
  - třídou, metodou, proměnnou,
  - záměrnou chybou pro diagnostiku.

## 8.3 Metriky „done“ pro první release

- LSP server startuje na čistém stroji s Java 11+.
- Otevření `.cls` v Zed aktivuje Apex language mode.
- Highlighting rozlišuje komentář/kód/string.
- Po syntaktické chybě se objeví alespoň jedna diagnostika z LSP.

## 9) Praktický plán realizace (po tomto dokumentačním kroku)

1. Scaffold extension (`extension.toml`, Rust crate, language config).
2. Přidat Tree-sitter grammar + minimal queries.
3. Implementovat `language_server_command` pro Java + JAR.
4. Přidat robustní detekci Java runtime + chybové hlášky.
5. Připravit `fixtures` a smoke test skripty.
6. Ověřit v Zed dev režimu na Linux/macOS/Windows.
7. Sepsat release notes + known limitations.

## 10) Poznámka k referenčnímu repozitáři Salesforce

Pro analýzu je vhodné mít lokálně referenci `forcedotcom/salesforcedx-vscode` (alespoň adresář `packages/salesforcedx-vscode-apex`).  
V omezených CI prostředích může `git clone` selhávat; alternativou je stažení ZIP snapshotu (`codeload.github.com`) jen pro studium.

---

## Stručné MVP zadání (executive summary)

- Dodat Zed extension pro Apex, která **spolehlivě spustí Apex LSP** (Java + `apex-jorje-lsp.jar`) a dá uživateli minimální, ale funkční jazykový komfort.
- V první verzi je důležitější **spolehlivý start LSP + základní highlighting** než široká feature sada.
- Dokumentace, testovatelnost a jasné provozní instrukce jsou součástí definice hotového MVP.
