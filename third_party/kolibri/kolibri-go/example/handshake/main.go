package main

import (
	"fmt"
	"log"

	kolibri "github.com/KometTeam/kolibri-go"
)

func main() {
	cfg := kolibri.DefaultConfig("api.oneme.ru")
	cfg.OnWire = func(e kolibri.WireEvent) {
		arrow := "<-"
		if e.Direction == "out" {
			arrow = "->"
		}
		js := e.JSON
		if len(js) > 90 {
			js = js[:90]
		}
		fmt.Printf("%s %-8s op=%d seq=%d %s\n", arrow, e.Cmd, e.Opcode, e.Seq, js)
	}

	s, err := kolibri.Open(cfg)
	if err != nil {
		log.Fatal(err)
	}
	defer s.Close()

	info, err := s.Connect()
	if err != nil {
		log.Fatal(err)
	}
	fmt.Printf("connected: state=%d, callsSeed=%v, location=%v\n",
		s.State(), info["callsSeed"], info["location"])

	fmt.Println("switching interactive=false on the live socket")
	s.SetPingInteractive(false)

	s.Disconnect()
}
