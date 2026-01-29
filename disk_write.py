data = bytearray(512)

# write a recognisable pattern

for i in range(512):
    data[i] = i % 256

with open('test_disk.img', 'r+b') as f:
    f.write(data)

print("Test pattern written to sector 0")

with open('test_disk.img', "rb")as f:
    sector = f.read(16)
    print(f"First 15 bytes: {' '.join(f'{b:02x}' for b in sector)}")