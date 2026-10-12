//go:build ignore

// Engine-managed Go build preparation. No verification decisions are made here.
package main

import (
	"crypto/sha256"
	"encoding/json"
	"fmt"
	"io"
	"os"
	"os/exec"
	"path/filepath"
	"strings"
	"syscall"
	"time"
)

type pkg struct {
	Dir                                                                       string
	ImportPath                                                                string
	Standard                                                                  bool
	GoFiles, CompiledGoFiles, SFiles, SysoFiles, HFiles, EmbedFiles, CgoFiles []string
	Module                                                                    *module
}
type module struct {
	Main       bool
	Dir, GoMod string
	Replace    *module
}
type receipt struct {
	SchemaVersion  int               `json:"schema_version"`
	GoVersion      string            `json:"go_version"`
	Files          map[string]string `json:"files"`
	Packages       []string          `json:"packages"`
	BinaryHash     string            `json:"binary_hash"`
	BuildElapsedUS uint64            `json:"build_elapsed_us"`
	BuildPeakRSS   *uint64           `json:"build_peak_rss_bytes"`
}

func hash(path string) (string, error) {
	file, err := os.Open(path)
	if err != nil {
		return "", err
	}
	defer file.Close()
	h := sha256.New()
	n, err := io.Copy(h, io.LimitReader(file, 268435457))
	if err != nil {
		return "", err
	}
	if n > 268435456 {
		return "", fmt.Errorf("Dependency file exceeds capture limit: %s", path)
	}
	return fmt.Sprintf("%x", h.Sum(nil)), nil
}

var compileElapsed uint64
var compileRSS *uint64

func command(args ...string) error {
	started := time.Now()
	cmd := exec.Command("go", args...)
	cmd.Stdout = os.Stderr
	cmd.Stderr = os.Stderr
	err := cmd.Run()
	compileElapsed = uint64(time.Since(started).Microseconds())
	if cmd.ProcessState != nil {
		if usage, ok := cmd.ProcessState.SysUsage().(*syscall.Rusage); ok && usage.Maxrss > 0 {
			bytes := uint64(usage.Maxrss) * 1024
			compileRSS = &bytes
		}
	}
	return err
}
func prepare() error {
	if err := os.MkdirAll(".bin", 0700); err != nil {
		return err
	}
	// go.mod JSON inspection is read-only. Reject replacements before vendoring
	// could obscure their original location in the captured project.
	// The application module owns dependency selection. Vendoring affects only its captured copy.
	root, err := exec.Command("go", "env", "GOMOD").Output()
	if err != nil {
		return err
	}
	modfile := strings.TrimSpace(string(root))
	if modfile == "" || modfile == os.DevNull {
		return fmt.Errorf("A Go application module is required")
	}
	moduleRoot := filepath.Dir(modfile)
	data, err := exec.Command("go", "mod", "edit", "-json").Output()
	if err != nil {
		return err
	}
	var config struct {
		Replace []struct {
			New struct{ Path, Version string }
		}
	}
	if err = json.Unmarshal(data, &config); err != nil {
		return err
	}
	for _, replacement := range config.Replace {
		if replacement.New.Version == "" {
			path := replacement.New.Path
			if !filepath.IsAbs(path) {
				path = filepath.Join(moduleRoot, path)
			}
			path = filepath.Clean(path)
			if path != moduleRoot && !strings.HasPrefix(path, moduleRoot+string(os.PathSeparator)) {
				return fmt.Errorf("External local replacements are unsupported: %s", path)
			}
		}
	}

	cmd := exec.Command("go", "mod", "vendor")
	cmd.Dir = moduleRoot
	cmd.Stdout = os.Stderr
	cmd.Stderr = os.Stderr
	if err = cmd.Run(); err != nil {
		return err
	}
	list := exec.Command("go", "list", "-mod=vendor", "-deps", "-json", ".")
	list.Stderr = os.Stderr
	stdout, err := list.StdoutPipe()
	if err != nil {
		return err
	}
	if err = list.Start(); err != nil {
		return err
	}
	r := receipt{SchemaVersion: 1, Files: map[string]string{}, Packages: []string{}}
	decoder := json.NewDecoder(io.LimitReader(stdout, 67108865))
	total := int64(0)
	capture := func(path string) error {
		if _, ok := r.Files[path]; ok {
			return nil
		}
		info, err := os.Lstat(path)
		if err != nil {
			return err
		}
		if !info.Mode().IsRegular() {
			return fmt.Errorf("Dependency is not a regular file: %s", path)
		}
		total += info.Size()
		if total > 536870912 || len(r.Files) >= 20000 {
			return fmt.Errorf("Dependency inventory exceeds limits")
		}
		sum, err := hash(path)
		if err != nil {
			return err
		}
		r.Files[path] = sum
		return nil
	}
	for {
		var p pkg
		err = decoder.Decode(&p)
		if err == io.EOF {
			break
		}
		if err != nil {
			list.Process.Kill()
			list.Wait()
			return err
		}
		if len(p.CgoFiles) > 0 {
			list.Process.Kill()
			list.Wait()
			return fmt.Errorf("CGO packages are unsupported: %s", p.ImportPath)
		}
		r.Packages = append(r.Packages, p.ImportPath)
		for _, files := range [][]string{p.GoFiles, p.CompiledGoFiles, p.SFiles, p.SysoFiles, p.HFiles, p.EmbedFiles} {
			for _, file := range files {
				if err = capture(filepath.Join(p.Dir, file)); err != nil {
					list.Process.Kill()
					list.Wait()
					return err
				}
			}
		}
		if p.Module != nil && p.Module.Replace != nil && !strings.HasPrefix(p.Dir, moduleRoot+string(os.PathSeparator)) {
			list.Process.Kill()
			list.Wait()
			return fmt.Errorf("External local replacements are unsupported")
		}
	}
	if err = list.Wait(); err != nil {
		return err
	}
	goPath, err := exec.LookPath("go")
	if err != nil {
		return err
	}
	if err = capture(goPath); err != nil {
		return err
	}
	tools, err := exec.Command("go", "env", "GOTOOLDIR").Output()
	if err != nil {
		return err
	}
	for _, tool := range []string{"compile", "asm", "link"} {
		if err = capture(filepath.Join(strings.TrimSpace(string(tools)), tool)); err != nil {
			return err
		}
	}
	if err = capture(modfile); err != nil {
		return err
	}
	for _, file := range []string{"go.sum", "vendor/modules.txt"} {
		path := filepath.Join(moduleRoot, file)
		if _, err := os.Stat(path); err == nil {
			if err = capture(path); err != nil {
				return err
			}
		}
	}
	goroot, err := exec.Command("go", "env", "GOROOT").Output()
	if err != nil {
		return err
	}
	headers, err := filepath.Glob(filepath.Join(strings.TrimSpace(string(goroot)), "pkg", "include", "*.h"))
	if err != nil {
		return err
	}
	for _, header := range headers {
		if err = capture(header); err != nil {
			return err
		}
	}
	version, err := exec.Command("go", "version").Output()
	if err != nil {
		return err
	}
	r.GoVersion = strings.TrimSpace(string(version))
	if err = command("build", "-trimpath", "-mod=vendor", "-o", ".bin/worker", "."); err != nil {
		return err
	}
	for path, before := range r.Files {
		after, err := hash(path)
		if err != nil {
			return err
		}
		if before != after {
			return fmt.Errorf("Build input changed: %s", path)
		}
	}
	r.BuildElapsedUS = compileElapsed
	r.BuildPeakRSS = compileRSS
	r.BinaryHash, err = hash(".bin/worker")
	if err != nil {
		return err
	}
	data, err = json.Marshal(r)
	if err != nil {
		return err
	}
	return os.WriteFile(".bin/dependencies.json", data, 0600)
}
func main() {
	if err := prepare(); err != nil {
		fmt.Fprintln(os.Stderr, err)
		os.Exit(1)
	}
}
