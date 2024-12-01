const globalVar = 42;
        function outer() {
            let outerVar = globalVar + 1;
            function inner() {
                const innerVar = outerVar * 2;
                return innerVar;
            }
            return inner() + outerVar;
        }
        class Example {
            constructor() {
                this.classVar = globalVar;
            }
            method() {
                return this.classVar + globalVar;
            }
        }