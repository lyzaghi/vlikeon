"""Build the original Vlike Grid Game Boy demo. No game/firmware download."""
import argparse
import hashlib
import json
from pathlib import Path

# Original 5x7 glyphs and graphics; each row is a five-bit bitmap.
FONT = {
 '0':[14,17,19,21,25,17,14], '1':[4,12,4,4,4,4,14],
 '2':[14,17,1,2,4,8,31], '3':[30,1,1,14,1,1,30],
 '4':[2,6,10,18,31,2,2], '5':[31,16,16,30,1,1,30],
 '6':[14,16,16,30,17,17,14], '7':[31,1,2,4,8,8,8],
 '8':[14,17,17,14,17,17,14], '9':[14,17,17,15,1,1,14],
 'A':[14,17,17,31,17,17,17], 'B':[30,17,17,30,17,17,30],
 'C':[14,17,16,16,16,17,14], 'D':[30,17,17,17,17,17,30],
 'E':[31,16,16,30,16,16,31], 'G':[14,17,16,23,17,17,15],
 'I':[14,4,4,4,4,4,14], 'K':[17,18,20,24,20,18,17],
 'L':[16,16,16,16,16,16,31], 'M':[17,27,21,21,17,17,17],
 'O':[14,17,17,17,17,17,14], 'P':[30,17,17,30,16,16,16],
 'R':[30,17,17,30,20,18,17], 'S':[15,16,16,14,1,1,30],
 'T':[31,4,4,4,4,4,4], 'U':[17,17,17,17,17,17,14],
 'V':[17,17,17,17,17,10,4], 'W':[17,17,17,21,21,21,10],
}

class Code:
    def __init__(self): self.data=bytearray(); self.labels={}; self.fixes=[]
    def emit(self,*values): self.data.extend(values)
    def label(self,name):
        if name in self.labels: raise ValueError('Duplicate ROM label')
        self.labels[name]=0x150+len(self.data)
    def word(self,value): self.emit(value&255,value>>8)
    def ref(self,opcode,name):
        self.emit(opcode); self.fixes.append((len(self.data),name)); self.word(0)
    def load(self,address): self.emit(0xfa); self.word(address)
    def store(self,address): self.emit(0xea); self.word(address)
    def imm(self,value): self.emit(0x3e,value)
    def io(self,address,value): self.imm(value); self.emit(0xe0,address)
    def finish(self):
        for at,name in self.fixes:
            value=self.labels[name]; self.data[at:at+2]=value.to_bytes(2,'little')
        if len(self.data)>0x7eb0: raise ValueError('Demo code exceeds its ROM bank')
        return self.data

def build():
    tiles=[bytes(16)]
    def tile(rows): return bytes(value for row in rows for value in (row,row))
    ids={}
    for character,rows in FONT.items(): ids[character]=len(tiles); tiles.append(tile([row<<2 for row in rows]+[0]))
    player=len(tiles); tiles.append(tile([0x18,0x3c,0x7e,0xff,0xff,0x7e,0x24,0x42]))
    orb=len(tiles); tiles.append(tile([0,0x18,0x3c,0x7e,0x7e,0x3c,0x18,0]))
    dot=len(tiles); tiles.append(tile([0,0,0,0x08,0,0,0,0]))
    background=bytearray(1024)
    for y in range(4,15):
        for x in range(2,18): background[y*32+x]=dot
    for x,y,text in [(5,1,'VLIKE GRID'),(4,2,'ORBS'),(3,16,'ARROWS TO MOVE')]:
        for i,character in enumerate(text): background[y*32+x+i]=ids.get(character,0)
    background[2*32+15]=ids['0']; background[2*32+16]=ids['0']
    c=Code(); c.label('boot'); c.emit(0xf3,0x31,0xfe,0xff)
    c.label('lcd_wait'); c.emit(0xf0,0x44,0xfe,144); c.ref(0xda,'lcd_wait')
    c.io(0x40,0); c.io(0x0f,0); c.io(0xff,0); c.io(0x42,0); c.io(0x43,0)
    for name,destination,length in [('tiles',0x8000,len(tiles)*16),('map',0x9800,1024)]:
        c.ref(0x21,name); c.emit(0x11); c.word(destination); c.emit(0x01); c.word(length)
        c.label('copy_'+name); c.emit(0x2a,0x12,0x13,0x0b,0x78,0xb1); c.ref(0xc2,'copy_'+name)
    c.emit(0x21,0,0xfe,0x06,160,0xaf); c.label('clear_oam'); c.emit(0x22,0x05); c.ref(0xc2,'clear_oam')
    for address,value in [(0xc000,3),(0xc001,8),(0xc002,15),(0xc003,8),(0xc004,0),(0xc005,0),(0xc006,57)]: c.imm(value); c.store(address)
    c.io(0x47,0xe4); c.io(0x48,0xe4); c.io(0x26,0x80); c.io(0x24,0x77); c.io(0x25,0x11); c.io(0x40,0x93)
    c.label('frame'); c.label('wait_active'); c.emit(0xf0,0x44,0xfe,144); c.ref(0xd2,'wait_active')
    c.label('wait_vblank'); c.emit(0xf0,0x44,0xfe,144); c.ref(0xda,'wait_vblank')
    c.load(0xc005); c.emit(0x3c,0xe6,7); c.store(0xc005); c.ref(0xc2,'draw')
    c.io(0,0x20); c.emit(0xf0,0,0xf0,0,0x2f,0xe6,15,0x57)
    for bit,address,bound,increase in [(0,0xc000,17,True),(1,0xc000,2,False),(2,0xc001,4,False),(3,0xc001,14,True)]:
        skip='skip_'+str(bit); c.emit(0xcb,0x42+8*bit); c.ref(0xca,skip)
        c.load(address); c.emit(0xfe,bound); c.ref(0xca,skip)
        c.emit(0x3c if increase else 0x3d); c.store(address); c.label(skip)
    c.io(0,0x30)
    c.load(0xc000); c.emit(0x47); c.load(0xc002); c.emit(0xb8); c.ref(0xc2,'draw')
    c.load(0xc001); c.emit(0x47); c.load(0xc003); c.emit(0xb8); c.ref(0xc2,'draw')
    c.load(0xc004); c.emit(0x3c,0xfe,100); c.ref(0xda,'score_ok'); c.emit(0xaf); c.label('score_ok'); c.store(0xc004)
    c.load(0xc006); c.emit(0x87,0x47,0x87,0x80,0xc6,17); c.store(0xc006); c.emit(0xe6,15,0xc6,2); c.store(0xc002)
    c.load(0xc006); c.emit(0x0f,0xe6,15); c.label('mod_y'); c.emit(0xfe,11); c.ref(0xda,'target_y'); c.emit(0xd6,11); c.ref(0xc3,'mod_y'); c.label('target_y'); c.emit(0xc6,4); c.store(0xc003)
    c.io(0x11,0x80); c.io(0x12,0xf1); c.io(0x13,0x80); c.io(0x14,0xc7)
    c.label('draw'); c.emit(0x21,0,0xfe)
    for address,tile_id in [(0xc000,player),(0xc002,orb)]:
        c.load(address+1); c.emit(0x87,0x87,0x87,0xc6,16,0x22)
        c.load(address); c.emit(0x87,0x87,0x87,0xc6,8,0x22)
        c.imm(tile_id); c.emit(0x22,0xaf,0x22)
    c.load(0xc004); c.emit(0x06,0); c.label('tens'); c.emit(0xfe,10); c.ref(0xda,'digits'); c.emit(0xd6,10,0x04); c.ref(0xc3,'tens')
    c.label('digits'); c.emit(0x4f,0x78,0xc6,ids['0'],0x21,0x4f,0x98,0x22,0x79,0xc6,ids['0'],0x22); c.ref(0xc3,'frame')
    c.label('tiles'); c.data.extend(b''.join(tiles)); c.label('map'); c.data.extend(background)
    rom=bytearray(32768); rom[0x100:0x104]=bytes([0,0xc3,0x50,1])
    # Hardware-format logo/checksum bytes; no manufacturer boot ROM or game assets.
    rom[0x104:0x134]=bytes.fromhex('CE ED 66 66 CC 0D 00 0B 03 73 00 83 00 0C 00 0D '
                                  '00 08 11 1F 88 89 00 0E DC CC 6E E6 DD DD D9 99 '
                                  'BB BB 67 63 6E 0E EC CC DD DC 99 9F BB B9 33 3E')
    rom[0x134:0x13d]=b'VLIKEGRID'; rom[0x14a]=1
    code=c.finish(); rom[0x150:0x150+len(code)]=code
    check=0
    for value in rom[0x134:0x14d]: check=(check-value-1)&255
    rom[0x14d]=check; checksum=sum(rom)&65535; rom[0x14e:0x150]=checksum.to_bytes(2,'big')
    return rom

def main():
    parser=argparse.ArgumentParser(description=__doc__); parser.add_argument('--output',type=Path,required=True); args=parser.parse_args()
    if args.output.exists(): raise ValueError('Choose a fresh demo ROM output')
    data=build(); args.output.parent.mkdir(parents=True,exist_ok=True); args.output.write_bytes(data)
    print(json.dumps({'game':'Vlike Grid','bytes':len(data),'sha256':hashlib.sha256(data).hexdigest(),'source':'tools/build_demo_rom.py','license':'Apache-2.0'}))

if __name__=='__main__': main()
