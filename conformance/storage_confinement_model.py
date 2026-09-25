#!/usr/bin/env python3
from itertools import product
# Regions: 0=app storage, 1=shared/external, 2=unknown

def main():
    checked=0
    for region,delete,owned in product(range(3),(False,True),(False,True)):
        allowed = (not delete) or (region==0 and owned)
        if delete and allowed:
            assert region==0,'delete admitted outside app storage'
            assert owned,'delete admitted for unowned artifact'
        checked+=1
    print(f'storage confinement model: {checked} states')
if __name__=='__main__':main()
