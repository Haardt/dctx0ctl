// Independent application fixture; no testing runtime or spec imports.
package main

import "errors"

// Counter is one stateful application service used by Go transport conformance.
type Counter struct {
	value   int
	load    func() (int, error)
	changed func(value int) error
}

func (counter *Counter) Increment(amount int) (int, error) {
	counter.value += amount
	if err := counter.changed(counter.value); err != nil {
		return 0, err
	}
	return counter.value, nil
}

func (counter *Counter) Crash() error {
	return errors.New("Unexpected application exception")
}

func (counter *Counter) Refresh() (int, error) {
	value, err := counter.load()
	if err != nil {
		return 0, err
	}
	counter.value = value
	if err := counter.changed(counter.value); err != nil {
		return 0, err
	}
	return counter.value, nil
}
