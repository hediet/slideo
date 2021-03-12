import * as webpack from "webpack";
import path = require("path");
import HtmlWebpackPlugin = require("html-webpack-plugin");
import CopyWebpackPlugin = require("copy-webpack-plugin");

const r = (file: string) => path.resolve(__dirname, file);

module.exports = {
	entry: { main: r("src/index.tsx"), viewer: r("src/viewer/index.ts") },
	output: {
		path: r("dist"),
		filename: "[name]-[fullhash].js",
		chunkFilename: "[name]-[fullhash].js",
	},
	resolve: {
		extensions: [".webpack.js", ".web.js", ".ts", ".tsx", ".js"],
	},
	devtool: "source-map",
	module: {
		rules: [
			{ test: /\.css$/, use: ["style-loader", "css-loader"] },
			{
				test: /\.scss$/,
				use: ["style-loader", "css-loader", "sass-loader"],
			},
			{
				test: /\.(jpe?g|png|gif|eot|ttf|svg|woff|woff2|md)$/i,
				loader: "file-loader",
			},
			{
				test: /\.tsx?$/,
				loader: "ts-loader",
				options: { transpileOnly: true },
			},
			{ test: /\.(png|gif|cur|jpg)$/, loader: "url-loader" },
		],
	},
	plugins: [
		new CopyWebpackPlugin({
			patterns: [
				{
					from: path.join(
						require.resolve("@hediet/pdfjs-viewer/package.json"),
						"../dist/assets"
					),
				},
			],
		}),
		new HtmlWebpackPlugin({
			chunks: ["main"],
		}),
		new HtmlWebpackPlugin({ chunks: ["viewer"], filename: "viewer.html" }),
	],
} as webpack.Configuration;
