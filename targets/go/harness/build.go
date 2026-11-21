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
	"reflect"
	"runtime"
	"strings"
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
	Discovery      map[string]string `json:"discovery,omitempty"`
	ModuleRoot     string            `json:"module_root,omitempty"`
}

// Versioned opt-in for read-only production modules used by an isolated fixture.
// The default still rejects every external local replacement. The compiler reads
// original source directly; receipts bind its selected inputs and package discovery.
func readonlyModules() (map[string]bool, error) {
	allowed := map[string]bool{}
	text := os.Getenv("DCTX0_GO_READONLY_MODULE_ROOTS_V1")
	if text == "" {
		return allowed, nil
	}
	var roots []string
	if len(text) > 65536 {
		return nil, fmt.Errorf("Read-only module declaration exceeds limit")
	}
	if err := json.Unmarshal([]byte(text), &roots); err != nil {
		return nil, err
	}
	if len(roots) > 128 {
		return nil, fmt.Errorf("Too many read-only modules")
	}
	for _, root := range roots {
		resolved, err := filepath.EvalSymlinks(root)
		if err != nil || !filepath.IsAbs(root) || filepath.Clean(root) != root || resolved != root {
			return nil, fmt.Errorf("Read-only module must be a canonical absolute directory: %s", root)
		}
		info, err := os.Stat(root)
		if err != nil || !info.IsDir() {
			return nil, fmt.Errorf("Read-only module is unavailable: %s", root)
		}
		allowed[root] = true
	}
	return allowed, nil
}

func discovery(directory string) (string, error) {
	entries, err := os.ReadDir(directory)
	if err != nil {
		return "", err
	}
	var inputs strings.Builder
	for _, entry := range entries {
		name := entry.Name()
		switch filepath.Ext(name) {
		case ".go", ".s", ".S", ".syso", ".h", ".c", ".cc", ".cpp", ".m", ".f", ".F", ".f90", ".swig", ".swigcxx":
			if entry.IsDir() {
				continue
			}
			// An existing excluded file can enter the build without being renamed.
			sum, err := hash(filepath.Join(directory, name))
			if err != nil {
				return "", err
			}
			inputs.WriteString(name + "\x00" + sum + "\x00")
		}
	}
	return fmt.Sprintf("%x", sha256.Sum256([]byte(inputs.String()))), nil
}

func moduleScope(directory string) (string, error) {
	var contents strings.Builder
	for _, name := range []string{"go.mod", "go.sum", "go.work", "go.work.sum"} {
		contents.WriteString(name + "\x00")
		sum, err := hash(filepath.Join(directory, name))
		if os.IsNotExist(err) {
			sum = "missing"
		} else if err != nil {
			return "", err
		}
		contents.WriteString(sum + "\x00")
	}
	return fmt.Sprintf("%x", sha256.Sum256([]byte(contents.String()))), nil
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
		// Rusage is Unix-only. Reflection keeps this single generated helper
		// buildable on Windows; unavailable metrics are explicitly null.
		usage := reflect.ValueOf(cmd.ProcessState.SysUsage())
		if (runtime.GOOS == "linux" || runtime.GOOS == "darwin") && usage.IsValid() && usage.Kind() == reflect.Pointer && !usage.IsNil() {
			value := usage.Elem().FieldByName("Maxrss")
			if value.IsValid() && value.CanInt() && value.Int() > 0 {
				bytes := uint64(value.Int())
				if runtime.GOOS == "linux" {
					bytes *= 1024
				}
				compileRSS = &bytes
			}
		}
	}
	return err
}
func exeSuffix() string {
	if runtime.GOOS == "windows" {
		return ".exe"
	}
	return ""
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
	allowed, err := readonlyModules()
	if err != nil {
		return err
	}
	external := false
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
				if !allowed[path] {
					return fmt.Errorf("External local replacements are unsupported: %s", path)
				}
				external = true
			}
		}
	}

	mode := "-mod=vendor"
	if external {
		mode = "-mod=readonly"
	} else {
		cmd := exec.Command("go", "mod", "vendor")
		cmd.Dir = moduleRoot
		cmd.Stdout = os.Stderr
		cmd.Stderr = os.Stderr
		if err = cmd.Run(); err != nil {
			return err
		}
	}
	list := exec.Command("go", "list", mode, "-deps", "-json", ".")
	list.Stderr = os.Stderr
	stdout, err := list.StdoutPipe()
	if err != nil {
		return err
	}
	if err = list.Start(); err != nil {
		return err
	}
	r := receipt{SchemaVersion: 1, Files: map[string]string{}, Packages: []string{}}
	if external {
		r.SchemaVersion = 2
		r.Discovery = map[string]string{}
		r.ModuleRoot = moduleRoot
	}
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
		if p.Module != nil {
			for _, module := range []*module{p.Module, p.Module.Replace} {
				if module != nil && module.GoMod != "" {
					if err = capture(module.GoMod); err != nil {
						list.Process.Kill()
						list.Wait()
						return err
					}
				}
			}
		}
		for _, files := range [][]string{p.GoFiles, p.CompiledGoFiles, p.SFiles, p.SysoFiles, p.HFiles, p.EmbedFiles} {
			for _, file := range files {
				if err = capture(filepath.Join(p.Dir, file)); err != nil {
					list.Process.Kill()
					list.Wait()
					return err
				}
			}
		}
		if p.Dir != moduleRoot && !strings.HasPrefix(p.Dir, moduleRoot+string(os.PathSeparator)) {
			sum, err := discovery(p.Dir)
			if err != nil {
				list.Process.Kill()
				list.Wait()
				return err
			}
			if external {
				r.Discovery[p.Dir] = sum
			}
		}
		if external && p.Module != nil && !p.Module.Main {
			root := p.Module.Dir
			if p.Module.Replace != nil {
				root = p.Module.Replace.Dir
			}
			for directory := p.Dir; ; directory = filepath.Dir(directory) {
				if directory != root && !strings.HasPrefix(directory, root+string(os.PathSeparator)) {
					list.Process.Kill()
					list.Wait()
					return fmt.Errorf("Package module scope escaped its resolved root")
				}
				sum, err := moduleScope(directory)
				if err != nil {
					list.Process.Kill()
					list.Wait()
					return err
				}
				r.Discovery["module-scope:"+directory] = sum
				if directory == root {
					break
				}
			}
		}
		if p.Module != nil && p.Module.Replace != nil && !strings.HasPrefix(p.Dir, moduleRoot+string(os.PathSeparator)) {
			root := filepath.Clean(p.Module.Replace.Dir)
			if !allowed[root] || (p.Dir != root && !strings.HasPrefix(p.Dir, root+string(os.PathSeparator))) || len(p.EmbedFiles) != 0 {
				list.Process.Kill()
				list.Wait()
				return fmt.Errorf("External module boundary or embedded resource is unsupported")
			}
			if err = capture(filepath.Join(root, "go.mod")); err != nil {
				list.Process.Kill()
				list.Wait()
				return err
			}
			sum, err := discovery(p.Dir)
			if err != nil {
				list.Process.Kill()
				list.Wait()
				return err
			}
			r.Discovery[p.Dir] = sum
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
		if err = capture(filepath.Join(strings.TrimSpace(string(tools)), tool+exeSuffix())); err != nil {
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
	for _, name := range []string{"go.env", "VERSION"} {
		path := filepath.Join(strings.TrimSpace(string(goroot)), name)
		if _, err := os.Stat(path); err == nil {
			if err = capture(path); err != nil {
				return err
			}
		}
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
	if err = command("build", "-trimpath", mode, "-o", ".bin/worker"+exeSuffix(), "."); err != nil {
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
	for directory, before := range r.Discovery {
		var after string
		var err error
		if strings.HasPrefix(directory, "module-scope:") {
			after, err = moduleScope(strings.TrimPrefix(directory, "module-scope:"))
		} else {
			after, err = discovery(directory)
		}
		if err != nil || before != after {
			return fmt.Errorf("Package discovery changed: %s", directory)
		}
	}
	r.BuildElapsedUS = compileElapsed
	r.BuildPeakRSS = compileRSS
	r.BinaryHash, err = hash(".bin/worker" + exeSuffix())
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
