package main

import (
	"fmt"
	"os"

	"github.com/eddix/wtp/cmd"
	"github.com/fatih/color"
)

func main() {
	if err := cmd.Execute(); err != nil {
		errStyle := color.New(color.FgRed)
		errStyle.Fprintf(os.Stderr, "Error: ")
		fmt.Fprintln(os.Stderr, err)
		os.Exit(1)
	}
}
