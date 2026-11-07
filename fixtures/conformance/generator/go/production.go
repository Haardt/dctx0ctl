// Independent production fixture; imports no dctx0 code.
package production

import (
	"bytes"
	"encoding/json"
	"math/big"
)

func Next(input *big.Int) *big.Int { return new(big.Int).Add(input, big.NewInt(1)) }

// ValidateCount is an independent production JSON boundary, not a generated rule.
func ValidateCount(raw []byte) bool {
	var value any
	decoder := json.NewDecoder(bytes.NewReader(raw))
	decoder.UseNumber()
	if decoder.Decode(&value) != nil {
		return false
	}
	number, ok := value.(json.Number)
	if !ok {
		return false
	}
	exact, ok := new(big.Int).SetString(number.String(), 10)
	return ok && exact.Cmp(big.NewInt(1)) >= 0 && exact.Cmp(big.NewInt(2)) <= 0
}
