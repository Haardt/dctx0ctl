package textcase
import "testing"
func TestCaseConversion(t *testing.T) {
 for _,c:=range []struct{in,upper,lower string}{{"Hello World","HELLO WORLD","hello world"},{"","",""},{"Grüße café","GRÜßE CAFÉ","grüße café"},{"a-B 123!","A-B 123!","a-b 123!"}} {
  if got:=Upper(c.in);got!=c.upper{t.Errorf("Upper(%q)=%q",c.in,got)}
  if got:=Lower(c.in);got!=c.lower{t.Errorf("Lower(%q)=%q",c.in,got)}
 }
}
