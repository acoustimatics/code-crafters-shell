```
Command       = BuiltIn Redirection?
              | External Redirection?

BuiltIn       = "cd" Word
              | "echo" (Integer | Word)*
              | "exit" Integer
              | "pwd"
              | "type" Word
        
External      = Word (Integer | Word)+

Redirection   = ">" Word
```