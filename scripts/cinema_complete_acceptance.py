#!/usr/bin/env python3
"""Complete TypeScript Cinema with retained authored, source-bound task reviews.

Called within validate.py, which pairs this acceptance with release measurements.
"""
import sys
from conformance_review_acceptance import main

if __name__ == '__main__':
    sys.argv.extend(['--target', 'typescript'])
    main()
