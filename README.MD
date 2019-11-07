# shortid


Example

```rust
use shortid::*;

fn to_string(src:&[u8])->String {
    src
        .into_iter()
        .map(|val| format!("{:0>2x}", val))
        .collect()
}

fn main() {

    let mac = [1,2,3,4,5,6];
    let epoch = 0;

    println!("{}" , to_string(&uuidv1(mac).unwrap()));

    let mac = [1,2,3,4];
    println!("{}" , to_string(&next_short_128(mac).unwrap()));

    let mac = [1,2,3];
    println!("{}" , to_string(&next_short_96(mac,epoch).unwrap()));

    println!("{}" , to_string(&next_short_64(epoch).unwrap()));

}
```

License: AGPL-3.0
