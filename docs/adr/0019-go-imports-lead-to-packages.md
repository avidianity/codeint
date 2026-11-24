---
status: accepted
---

# A Go import leads to every file of a Go package

The Owner decided this on 2026-11-24 while planning Go support.

## Context

In Go, an import names a Go package, which is one folder of files.
The files of one Go package see each other with no import between them.
In codeint, one runtime import leads to exactly one file, and Import cycles and Orphan files treat every file as a module.
No Language plugin resolves an import to several files today.

A Go program starts in a folder whose files declare `package main`, and no manifest lists these folders.
A Language plugin finds entry points from file paths and manifests only, so it cannot read the package clause at that step.

Go lets code outside a module import every Go package of that module, except a Go package under a folder named `internal`.
Go's compiler rejects every import cycle between Go packages.

Checked on 2026-11-24 against the Go specification for language version go1.27, and the `cmd/go` documentation at go1.27.4.

## Decision

**One runtime import may lead to several files, and a Go import leads to every file of the Go package that it names.**
An import never leads to a file whose name ends in `_test.go`, because Go does not let other code import test files.
A Language plugin may mark a file as a program start when it reads the file.
The Go Language plugin marks every file of a folder that declares `package main`.
Every Go package outside an `internal` folder is an entry point, because code outside the repository may import it.
Import cycles shows its measured value for Go, which is 0 for almost every repository.
Both additions to the Plugin interface ship in the same new interface version as the additions that ADR 0016, ADR 0017, and ADR 0018 need.

## Considered options

- **Keep one file per import, and invent a link from each file to the next file of its folder.**
  Rejected: a Finding would name a link that no source line contains, and an Import boundary Finding would name the first file of a folder, not the Go package.
- **Guess program starts from paths, such as a file named `main.go` or a folder under `cmd`.**
  Rejected: it misses every program that follows neither habit, and Orphan files would then report that program as unused.
- **Find no entry points for Go, and let the user list them.**
  Rejected: Orphan files would be off for Go until the user configures it.
- **Count only programs as entry points.**
  Rejected: a library would be reported as almost all Orphan files.
- **Report Import cycles as absent for Go, because the compiler forbids them.**
  Rejected: the value is a true measurement, and "absent" means that a question does not apply.

## Consequences

- Go support waits for the new Plugin interface version.
- The built-in Language plugins for PHP and Python can later model an import of a whole folder in the same way.
- An application that keeps unused Go packages outside `internal` gets no Orphan file Finding for them.
- The program-start mark decides only for programs under `internal`, such as code generators, because every other folder is already an entry point.
- codeint evaluates no build conditions, so it can report an Import cycle that runs through files for different operating systems, which the compiler never sees.
- The definition of Orphan file in the glossary names entry points from manifests and configuration only, and changes when Go ships.
