/*
 * Copyright (C) 2025 Karere Contributors
 *
 * This program is free software: you can redistribute it and/or modify
 * it under the terms of the GNU General Public License as published by
 * the Free Software Foundation, either version 3 of the License, or
 * (at your option) any later version.
 *
 * This program is distributed in the hope that it will be useful,
 * but WITHOUT ANY WARRANTY; without even the implied warranty of
 * MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
 * GNU General Public License for more details.
 */

int main(string[] args) {
    Test.init(ref args);
    
    // Register all test suites
    KarereTests.register_application_tests();
    KarereTests.register_window_tests();
    
    return Test.run();
}