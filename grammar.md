```
Command       = SimpleCommand Redirection?

SimpleCommand = BuiltIn
              | External

BuiltIn       = "cd" Word
              | "echo" (Integer | Word)*
              | "exit" Integer
              | "pwd"
              | "type" Word
        
External      = Word (Integer | Word)+

Redirection   = ">" Word
```