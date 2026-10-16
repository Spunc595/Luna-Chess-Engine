# Release build for Windows x86-64, baseline target -- explicit, verified, nothing left to a default that can
# silently change (see .cargo/config.toml's own comment, and the v4.1.0 incident this script exists to prevent:
# the shipped Windows binary was built with target-cpu=native unconditionally, with no indication in the
# binary's own behavior that anything was wrong -- only a disassembly caught it, after the fact).
#
# What this does NOT fully solve: this binary is stripped (LTO + default release profile), so there is no
# reliable way to attribute a disassembled instruction to "which source function it came from" -- real function
# boundaries from .pdata unwind info were tried and found to not line up with source-level functions after
# aggressive inlining (one ~300KB blob contains nearly everything). The checks below are therefore global counts,
# not a true "outside the runtime-detected kernel" check: popcnt/lzcnt/BMI1/BMI2 get a zero-tolerance gate (this
# codebase never uses them intentionally, baseline or not, so ANY occurrence means the build was not baseline);
# AVX2 (`%ymm` registers) gets a generous ceiling instead of zero, because the legitimate runtime-detected AVX2
# kernel in src/nnue.rs is expected to contain some -- verified baseline builds in this project showed roughly
# 120-280 occurrences for that kernel alone, so the ceiling here is set well above that, not at zero.

$ErrorActionPreference = "Stop"

Write-Host "=== RUSTFLAGS esplicito (sovrascrive .cargo/config.toml, qualunque cosa contenga) ==="
$env:RUSTFLAGS = ""
Write-Host "RUSTFLAGS=[$env:RUSTFLAGS]"

Write-Host "`n=== cfg effettivo (tramite cargo, non rustc nudo -- rustc da solo non vede .cargo/config.toml) ==="
$cfgOutput = cargo rustc --release --bin luna -- --print cfg
$cfgOutput | Select-String "target_feature"
$features = ($cfgOutput | Select-String "target_feature") | Measure-Object | Select-Object -ExpandProperty Count
$expected = @("cmpxchg16b", "fxsr", "sse", "sse2", "sse3")
$unexpected = ($cfgOutput | Select-String "target_feature" | Where-Object {
    $f = ($_ -replace '.*target_feature="([^"]+)".*', '$1')
    $expected -notcontains $f
})
if ($unexpected) {
    Write-Error "cfg mostra target_feature oltre il baseline atteso (sse/sse2/sse3/cmpxchg16b/fxsr): $unexpected -- build NON baseline, blocco."
    exit 1
}
Write-Host "OK: solo le $($expected.Count) target_feature attese del baseline x86-64."

Write-Host "`n=== build ==="
cargo build --release
if ($LASTEXITCODE -ne 0) { Write-Error "cargo build fallita"; exit 1 }

$exe = "target\release\luna.exe"
$objdumpCandidates = Get-ChildItem "$env:USERPROFILE\.rustup\toolchains\*\lib\rustlib\x86_64-pc-windows-msvc\bin\llvm-objdump.exe" -ErrorAction SilentlyContinue
if (-not $objdumpCandidates) {
    Write-Error "llvm-objdump non trovato (componente llvm-tools di rustup) -- impossibile verificare il disassemblato, blocco."
    exit 1
}
$objdump = $objdumpCandidates[0].FullName

Write-Host "`n=== disassemblo $exe ==="
$disasmFile = "release_disasm_tmp.txt"
& $objdump -d --no-show-raw-insn $exe | Out-File -Encoding utf8 $disasmFile

Write-Host "`n=== controllo: zero popcnt/lzcnt/BMI1/BMI2 (tolleranza zero, mai usate volutamente in questo codice) ==="
$bad = Select-String -Path $disasmFile -Pattern '\b(popcnt|lzcnt|andn|blsr|blsi|blsmsk|shlx|shrx|sarx|bzhi|mulx|rorx)[lqwb]?\s'
if ($bad) {
    Write-Error "TROVATE $($bad.Count) istruzioni popcnt/lzcnt/BMI1/BMI2 -- build NON baseline. Prime righe:"
    $bad | Select-Object -First 5 | ForEach-Object { Write-Host $_.Line }
    Remove-Item $disasmFile
    exit 1
}
Write-Host "OK: nessuna istruzione popcnt/lzcnt/BMI1/BMI2 trovata."

Write-Host "`n=== controllo: uso di AVX2 (%ymm) entro un tetto generoso (atteso: solo il kernel con gate a runtime) ==="
$ymmCount = (Select-String -Path $disasmFile -Pattern '%ymm[0-9]+' -AllMatches | ForEach-Object { $_.Matches.Count } | Measure-Object -Sum).Sum
$ymmCeiling = 500
Write-Host "occorrenze %ymm: $ymmCount (tetto: $ymmCeiling)"
if ($ymmCount -gt $ymmCeiling) {
    Write-Error "%ymm usato $ymmCount volte, oltre il tetto di $ymmCeiling -- probabile AVX2 globale, non confinato al kernel con gate a runtime. Blocco."
    Remove-Item $disasmFile
    exit 1
}
Write-Host "OK: uso di AVX2 coerente con il solo kernel a rilevazione runtime."

Remove-Item $disasmFile
Write-Host "`n=== fatto: $exe e' una build baseline x86-64 verificata ==="
