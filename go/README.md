# ttid Go module

Import path:

```go
import "github.com/theduke/ttid/go"
```

Typical use:

```go
codec := ttid.NewCodec(MyDomain{})
id, _ := codec.New(ttid.User)
fmt.Println(id.String())
```
