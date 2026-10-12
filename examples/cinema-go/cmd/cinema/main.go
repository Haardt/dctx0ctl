package main

import (
	"cinema/application"
	"cinema/ginapi"
	"cinema/httpapi"
	"cinema/persistence"
	"crypto/rand"
	"fmt"
	"log"
	"net/http"
	"os"
)

func main() {
	database := os.Getenv("CINEMA_DATABASE")
	if database == "" {
		database = "cinema.sqlite3"
	}
	store, err := persistence.New(database, persistence.Defaults(func() string {
		var id [16]byte
		if _, err := rand.Read(id[:]); err != nil {
			panic(err)
		}
		id[6] = id[6]&15 | 64
		id[8] = id[8]&63 | 128
		return fmt.Sprintf("%x-%x-%x-%x-%x", id[0:4], id[4:6], id[6:8], id[8:10], id[10:16])
	}), nil)
	if err != nil {
		log.Fatal(err)
	}
	defer store.Close()
	service := application.New(store, func(event application.Event) error { log.Printf("%s", event.Name); return nil })
	address := os.Getenv("CINEMA_ADDRESS")
	if address == "" {
		address = "127.0.0.1:4181"
	}
	var handler http.Handler
	switch os.Getenv("CINEMA_HTTP_FRAMEWORK") {
	case "", "net-http":
		handler = httpapi.New(service)
	case "gin":
		handler = ginapi.New(service)
	default:
		log.Fatal("CINEMA_HTTP_FRAMEWORK must be net-http or gin")
	}
	log.Fatal(http.ListenAndServe(address, handler))
}
