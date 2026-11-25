package main

import (
	"bufio"
	"encoding/json"
	"go/ast"
	"go/parser"
	"go/token"
	"os"
	"strconv"
)

type fileReport struct {
	Path      string   `json:"path"`
	Functions [][3]int `json:"functions"`
	Imports   []string `json:"imports"`
	Error     string   `json:"error,omitempty"`
}

func oneWhen(condition bool) int {
	if condition {
		return 1
	}
	return 0
}

func decisionWeight(node ast.Node) int {
	switch n := node.(type) {
	case *ast.IfStmt, *ast.ForStmt, *ast.RangeStmt:
		return 1
	case *ast.CaseClause:
		return oneWhen(len(n.List) > 0)
	case *ast.CommClause:
		return oneWhen(n.Comm != nil)
	case *ast.BinaryExpr:
		return oneWhen(n.Op == token.LAND || n.Op == token.LOR)
	}
	return 0
}

func decisions(body *ast.BlockStmt) int {
	total := 0
	ast.Inspect(body, func(node ast.Node) bool {
		total += decisionWeight(node)
		return true
	})
	return total
}

func imports(file *ast.File) []string {
	found := []string{}
	for _, spec := range file.Imports {
		if value, err := strconv.Unquote(spec.Path.Value); err == nil {
			found = append(found, value)
		}
	}
	return found
}

func describe(path string) fileReport {
	files := token.NewFileSet()
	file, err := parser.ParseFile(files, path, nil, parser.SkipObjectResolution)
	if err != nil {
		return fileReport{Path: path, Error: err.Error()}
	}
	report := fileReport{Path: path, Functions: [][3]int{}, Imports: imports(file)}
	for _, declaration := range file.Decls {
		function, ok := declaration.(*ast.FuncDecl)
		if !ok || function.Body == nil {
			continue
		}
		start := files.Position(function.Pos()).Line
		end := files.Position(function.End()).Line
		report.Functions = append(report.Functions, [3]int{start, end, decisions(function.Body)})
	}
	return report
}

func main() {
	listing, err := os.Open(os.Args[1])
	if err != nil {
		os.Exit(2)
	}
	defer listing.Close()
	encoder := json.NewEncoder(os.Stdout)
	scanner := bufio.NewScanner(listing)
	for scanner.Scan() {
		if path := scanner.Text(); path != "" {
			_ = encoder.Encode(describe(path))
		}
	}
}
