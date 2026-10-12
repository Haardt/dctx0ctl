package main

import (
	"casecli/textcase"
	"fmt"
	"os"
	"strings"
)

func main() {
	if len(os.Args) < 3 {
		fmt.Fprintln(os.Stderr, "usage: casecli upper|lower TEXT")
		os.Exit(2)
	}
	input := strings.Join(os.Args[2:], " ")
	switch os.Args[1] {
	case "upper":
		fmt.Println(textcase.Upper(input))
	case "lower":
		fmt.Println(textcase.Lower(input))
	default:
		fmt.Fprintln(os.Stderr, "unknown command")
		os.Exit(2)
	}
}
